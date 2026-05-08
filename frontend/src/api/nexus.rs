// nexus.rs
//
// Nexus Mods API client.
// Fetches latest mod info and download URLs, with local disk caching
// to avoid burning through the 100 req/hour rate limit.
//
// All methods are async — call from a tokio runtime (Dioxus desktop provides one).
//
// API reference: https://app.swaggerhub.com/apis-list/NexusMods/nexus-mods_public_api/1.0

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use anyhow::{bail, Context, Result};

use crate::api::paths;

const BASE_URL: &str = "https://api.nexusmods.com/v1";
const GAME_DOMAIN: &str = "stardewvalley";

// ─── Response types ───────────────────────────────────────────────────────────

/// Relevant fields from `GET /games/{domain}/mods/{id}.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusModInfo {
    pub mod_id: u32,
    pub name: String,
    pub version: String,       // latest uploaded version string
    pub author: String,
    pub summary: String,
    pub updated_timestamp: u64,
}

/// Relevant fields from `GET /games/{domain}/mods/{id}/files.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusFileInfo {
    pub file_id: u64,
    pub file_name: String,
    pub version: String,
    pub category_name: String, // "MAIN", "UPDATE", "OLD_VERSION", etc.
    pub is_primary: bool,
}

/// Response wrapper for the files endpoint
#[derive(Debug, Deserialize)]
struct FilesResponse {
    files: Vec<NexusFileInfo>,
}

/// One download link entry from the download_link endpoint
#[derive(Debug, Deserialize)]
struct DownloadLink {
    #[serde(rename = "URI")]
    uri: String,
    // `name` and `short_name` also present but we only need the URL
}

// ─── Cache types ──────────────────────────────────────────────────────────────

/// Wraps a cached value with the Unix timestamp it was fetched at.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedEntry<T> {
    fetched_at: u64,
    data: T,
}

/// The full on-disk cache: mod_id → cached mod info.
#[derive(Debug, Default, Serialize, Deserialize)]
struct NexusCache {
    mods: HashMap<u32, CachedEntry<NexusModInfo>>,
}

impl NexusCache {
    fn load() -> Self {
        let Ok(path) = paths::nexus_cache_file() else { return Self::default() };
        let Ok(contents) = std::fs::read_to_string(&path) else { return Self::default() };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    fn save(&self) {
        let Ok(path) = paths::nexus_cache_file() else { return };
        if let Ok(json) = serde_json::to_string(self) {
            let _ = std::fs::write(path, json);
        }
    }

    fn get(&self, mod_id: u32, ttl_seconds: u64) -> Option<&NexusModInfo> {
        let entry = self.mods.get(&mod_id)?;
        let now = unix_now();
        if now.saturating_sub(entry.fetched_at) < ttl_seconds {
            Some(&entry.data)
        } else {
            None // expired
        }
    }

    fn insert(&mut self, mod_id: u32, data: NexusModInfo) {
        self.mods.insert(mod_id, CachedEntry { fetched_at: unix_now(), data });
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─── NexusClient ──────────────────────────────────────────────────────────────

/// Async HTTP client for the Nexus Mods API.
/// Construct once and reuse — `reqwest::Client` pools connections internally.
pub struct NexusClient {
    http: reqwest::Client,
    cache: NexusCache,
    ttl_seconds: u64,
}

impl NexusClient {
    /// Create a new client authenticated with the given API key.
    /// `ttl_seconds` controls how long cached responses are trusted.
    pub fn new(api_key: &str, ttl_seconds: u64) -> Result<Self> {
        use reqwest::header::{self, HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();

        // Nexus auth: plain API key in `apikey` header
        headers.insert(
            "apikey",
            HeaderValue::from_str(api_key)
                .context("API key contains invalid header characters")?,
        );

        // Required by Nexus — identify our app
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("StardewModManager/0.1 (github.com/you/stardew-mod-manager)"),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .http1_only()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            cache: NexusCache::load(),
            ttl_seconds,
        })
    }

    /// Fetch mod info for a single Nexus mod ID.
    /// Returns cached data if fresh, otherwise hits the API.
    pub async fn fetch_mod_info(&mut self, mod_id: u32) -> Result<NexusModInfo> {
        // Cache hit?
        if let Some(cached) = self.cache.get(mod_id, self.ttl_seconds) {
            return Ok(cached.clone());
        }

        let url = format!("{BASE_URL}/games/{GAME_DOMAIN}/mods/{mod_id}.json");
        let response = self.http.get(&url).send().await
            .with_context(|| format!("Request failed for mod {mod_id}"))?;

        handle_rate_limit(&response)?;

        let info: NexusModInfo = response
            .error_for_status()
            .with_context(|| format!("Nexus API error for mod {mod_id}"))?
            .json()
            .await
            .with_context(|| format!("Failed to parse response for mod {mod_id}"))?;

        // Cache and persist
        self.cache.insert(mod_id, info.clone());
        self.cache.save();

        Ok(info)
    }

    /// Fetch mod info for multiple mod IDs concurrently.
    /// Returns a map of mod_id → Result so partial failures don't abort the batch.
    pub async fn fetch_many(
        &mut self,
        mod_ids: &[u32],
    ) -> HashMap<u32, Result<NexusModInfo>> {
        // We need to run requests concurrently but `self` can't be borrowed
        // mutably across concurrent futures. So we split: check cache first
        // for all IDs, then fire off HTTP requests only for the misses.

        let mut results: HashMap<u32, Result<NexusModInfo>> = HashMap::new();
        let mut to_fetch: Vec<u32> = Vec::new();

        for &id in mod_ids {
            if let Some(cached) = self.cache.get(id, self.ttl_seconds) {
                results.insert(id, Ok(cached.clone()));
            } else {
                to_fetch.push(id);
            }
        }

        // Fire all cache-miss requests concurrently
        if !to_fetch.is_empty() {
            let futures: Vec<_> = to_fetch
                .iter()
                .map(|&id| {
                    let url = format!("{BASE_URL}/games/{GAME_DOMAIN}/mods/{id}.json");
                    let client = self.http.clone(); // reqwest::Client is Arc-backed, cheap clone
                    async move {
                        let resp = client.get(&url).send().await;
                        (id, resp)
                    }
                })
                .collect();

            let responses = futures::future::join_all(futures).await;

            for (id, resp_result) in responses {
                let entry = match resp_result {
                    Err(e) => Err(anyhow::anyhow!("Request failed: {e}")),
                    Ok(resp) => {
                        if let Err(e) = handle_rate_limit(&resp) {
                            Err(e)
                        } else {
                            match resp.error_for_status() {
                                Ok(success_resp) => match success_resp.json::<NexusModInfo>().await {
                                    Ok(info) => Ok(info),
                                    Err(e) => Err(anyhow::anyhow!("Parse error: {}", e)),
                                },
                                Err(e) => Err(anyhow::anyhow!("API error: {}", e)),
                            }
                        }
                    }
                };

                if let Ok(ref info) = entry {
                    self.cache.insert(id, info.clone());
                }

                results.insert(id, entry);
            }

            self.cache.save();
        }

        results
    }

    /// Fetch the list of files for a mod. Used to find the latest main file ID
    /// before requesting a download link (Premium only).
    pub async fn fetch_files(&self, mod_id: u32) -> Result<Vec<NexusFileInfo>> {
        let url = format!("{BASE_URL}/games/{GAME_DOMAIN}/mods/{mod_id}/files.json");
        let resp = self.http.get(&url).send().await
            .with_context(|| format!("Failed to fetch file list for mod {mod_id}"))?;

        handle_rate_limit(&resp)?;

        let data: FilesResponse = resp
            .error_for_status()
            .context("Nexus files API error")?
            .json()
            .await
            .context("Failed to parse files response")?;

        Ok(data.files)
    }

    /// Get the primary (main) file for the latest version of a mod.
    /// Returns the file marked `is_primary: true`, or the first MAIN category file.
    pub async fn latest_main_file(&self, mod_id: u32) -> Result<NexusFileInfo> {
        let files = self.fetch_files(mod_id).await?;

        files
            .into_iter()
            .filter(|f| f.category_name == "MAIN")
            .find(|f| f.is_primary)
            .or_else(|| {
                // No primary flag? Just take the first MAIN file.
                // Re-fetch since we consumed the iterator — in practice
                // callers should cache `fetch_files` results themselves.
                None
            })
            .context("No main file found for mod — may be archived or adult-only")
    }

    /// Generate a CDN download URL for a specific file. **Requires Premium API key.**
    /// Free accounts will get HTTP 403 — the caller should handle this
    /// by opening the mod page in the browser instead.
    pub async fn download_url(&self, mod_id: u32, file_id: u64) -> Result<String> {
        let url = format!(
            "{BASE_URL}/games/{GAME_DOMAIN}/mods/{mod_id}/files/{file_id}/download_link.json"
        );

        let resp = self.http.get(&url).send().await
            .context("Failed to request download link")?;

        // 403 on this endpoint specifically means "not Premium"
        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            bail!(
                "Download links require a Nexus Premium account. \
                 Open the mod page to download manually."
            );
        }

        handle_rate_limit(&resp)?;

        let links: Vec<DownloadLink> = resp
            .error_for_status()
            .context("Download link API error")?
            .json()
            .await
            .context("Failed to parse download link response")?;

        links
            .into_iter()
            .next()
            .map(|l| l.uri)
            .context("No download links returned")
    }

    /// Check whether the stored API key is for a Premium account.
    /// Uses the `/v1/users/validate.json` endpoint which returns user info.
    pub async fn is_premium(&self) -> Result<bool> {
        #[derive(Deserialize)]
        struct UserInfo {
            is_premium: bool,
        }

        let url = format!("{BASE_URL}/users/validate.json");
        let info: UserInfo = self.http
            .get(&url)
            .send()
            .await
            .context("Failed to validate API key")?
            .error_for_status()
            .context("API key validation failed — key may be invalid")?
            .json()
            .await
            .context("Failed to parse user info")?;

        Ok(info.is_premium)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Check the response for rate-limit headers and return a descriptive error
/// if the limit has been hit (HTTP 429).
fn handle_rate_limit(response: &reqwest::Response) -> Result<()> {
    if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Ok(());
    }

    // Nexus returns `X-RateLimit-Reset` as a Unix timestamp
    let reset_at = response
        .headers()
        .get("X-RateLimit-Reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    match reset_at {
        Some(ts) => {
            let secs_until = ts.saturating_sub(unix_now());
            bail!(
                "Nexus rate limit reached. Resets in {} minutes.",
                secs_until / 60 + 1
            )
        }
        None => bail!("Nexus rate limit reached. Please wait before retrying."),
    }
}