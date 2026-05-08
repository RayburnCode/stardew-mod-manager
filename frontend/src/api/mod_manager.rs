// mods.rs
//
// Mod discovery and version comparison.
// Reads installed mods by scanning the Mods/ folder for manifest.json files,
// then compares installed versions against latest from Nexus.
//
// No network calls here — that's nexus.rs. No UI — that's ui/.
// This module is pure filesystem + data logic, fully testable offline.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

// ─── Data types ───────────────────────────────────────────────────────────────

/// Deserialized from each mod's `manifest.json`.
/// Fields match SMAPI's manifest spec exactly.
/// Unknown fields are ignored via `deny_unknown_fields = false` (default).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ModManifest {
    pub name: String,
    pub author: String,
    pub version: String,

    #[serde(default)]
    pub description: String,

    pub unique_id: String,

    /// e.g. ["Nexus:2400", "GitHub:Pathoschild/SMAPI"]
    /// We only act on "Nexus:XXXX" entries — others are ignored.
    #[serde(default)]
    pub update_keys: Vec<String>,
}

impl ModManifest {
    /// Extract the Nexus mod ID from UpdateKeys, if present.
    /// Returns the first valid `Nexus:XXXX` entry parsed as u32.
    pub fn nexus_id(&self) -> Option<u32> {
        self.update_keys
            .iter()
            .find_map(|key| {
                key.strip_prefix("Nexus:")
                    .and_then(|id| id.parse::<u32>().ok())
            })
    }
}

/// A discovered mod: its manifest plus where it lives on disk.
#[derive(Debug, Clone)]
pub struct InstalledMod {
    pub manifest: ModManifest,
    /// The mod's own subfolder, e.g. `.../Mods/CJB Cheats Menu/`
    pub path: PathBuf,
    /// Computed after comparing with Nexus data
    pub status: ModStatus,
}

/// What we know about this mod's update state.
#[derive(Debug, Clone, PartialEq)]
pub enum ModStatus {
    /// Installed version matches latest on Nexus.
    UpToDate,
    /// Nexus has a newer version.
    UpdateAvailable { latest: String },
    /// No Nexus update key, or Nexus fetch hasn't run yet.
    Unknown,
    /// Nexus fetch failed for this mod specifically.
    FetchFailed { reason: String },
}

// ─── Discovery ────────────────────────────────────────────────────────────────

/// Scan `mods_path` and return one `InstalledMod` per valid subdirectory.
///
/// Subdirectories with no `manifest.json`, or with unparseable JSON,
/// are logged as warnings and skipped — they never cause an error return.
/// SMAPI itself lives in the Mods folder but is skipped (UniqueID check).
pub fn discover_mods(mods_path: &Path) -> Result<Vec<InstalledMod>> {
    let entries = std::fs::read_dir(mods_path)
        .with_context(|| format!("Cannot read Mods folder: {}", mods_path.display()))?;

    let mut mods = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[mods] Skipping unreadable entry: {e}");
                continue;
            }
        };

        let path = entry.path();

        // Only look at directories
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            eprintln!("[mods] No manifest.json in {}, skipping", path.display());
            continue;
        }

        match parse_manifest(&manifest_path) {
            Ok(manifest) => {
                // Skip SMAPI itself — it's in the Mods folder but isn't a mod
                if manifest.unique_id == "Pathoschild.SMAPI" {
                    continue;
                }

                mods.push(InstalledMod {
                    manifest,
                    path,
                    status: ModStatus::Unknown,
                });
            }
            Err(e) => {
                eprintln!("[mods] Failed to parse {}: {e}", manifest_path.display());
            }
        }
    }

    // Sort alphabetically by name for consistent display order
    mods.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));

    Ok(mods)
}

/// Parse a single `manifest.json` file into a `ModManifest`.
fn parse_manifest(path: &Path) -> Result<ModManifest> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    // SMAPI manifests sometimes have trailing commas or comments.
    // serde_json is strict, so we use json5 as a fallback if strict parse fails.
    serde_json::from_str(&contents)
        .or_else(|_| json5::from_str(&contents))
        .with_context(|| format!("Invalid JSON in {}", path.display()))
}

// ─── Version comparison ───────────────────────────────────────────────────────
//
// SMAPI versions are *almost* semver but not always: "1.6", "1.6.0", "1.6.0.0",
// "1.6.0-beta.1". We use the `semver` crate but pre-process the string to make
// it compliant before parsing.

/// Returns true if `latest` is strictly newer than `installed`.
///
/// Falls back to string equality if either version can't be parsed —
/// an unparseable version is never considered "newer".
pub fn is_update_available(installed: &str, latest: &str) -> bool {
    let Some(inst) = parse_version(installed) else { return false };
    let Some(lat)  = parse_version(latest)    else { return false };
    lat > inst
}

/// Parse a potentially non-standard version string into `semver::Version`.
/// Handles: "1.6" → "1.6.0", "1.6.0.0" → "1.6.0", "1.6.0-beta.1" → kept as-is.
fn parse_version(raw: &str) -> Option<semver::Version> {
    let normalized = normalize_version(raw);
    semver::Version::parse(&normalized).ok()
}

fn normalize_version(raw: &str) -> String {
    // Strip any leading 'v' or 'V'
    let s = raw.trim().trim_start_matches(['v', 'V']);

    // Split on '-' to separate version from pre-release tag
    let (version_part, pre) = match s.split_once('-') {
        Some((v, p)) => (v, Some(p)),
        None => (s, None),
    };

    // Pad with .0s until we have exactly 3 numeric segments
    let parts: Vec<&str> = version_part.split('.').collect();
    let normalized = match parts.len() {
        0 => "0.0.0".to_string(),
        1 => format!("{}.0.0", parts[0]),
        2 => format!("{}.{}.0", parts[0], parts[1]),
        3 => format!("{}.{}.{}", parts[0], parts[1], parts[2]),
        // 4-part versions (e.g. 1.6.0.0) — drop the 4th segment
        _ => format!("{}.{}.{}", parts[0], parts[1], parts[2]),
    };

    match pre {
        Some(p) => format!("{normalized}-{p}"),
        None => normalized,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nexus_id_parses_correctly() {
        let manifest = ModManifest {
            name: "Test".into(),
            author: "Author".into(),
            version: "1.0.0".into(),
            description: "".into(),
            unique_id: "Test.Mod".into(),
            update_keys: vec!["Nexus:2400".into(), "GitHub:foo/bar".into()],
        };
        assert_eq!(manifest.nexus_id(), Some(2400));
    }

    #[test]
    fn nexus_id_returns_none_when_absent() {
        let manifest = ModManifest {
            name: "Test".into(),
            author: "Author".into(),
            version: "1.0.0".into(),
            description: "".into(),
            unique_id: "Test.Mod".into(),
            update_keys: vec!["GitHub:foo/bar".into()],
        };
        assert!(manifest.nexus_id().is_none());
    }

    #[test]
    fn version_comparison_basic() {
        assert!(is_update_available("1.0.0", "1.1.0"));
        assert!(!is_update_available("1.1.0", "1.0.0"));
        assert!(!is_update_available("1.0.0", "1.0.0"));
    }

    #[test]
    fn version_comparison_with_padding() {
        // "1.6" should parse as "1.6.0"
        assert!(is_update_available("1.6", "1.7.0"));
        assert!(!is_update_available("1.7", "1.6.0"));
    }

    #[test]
    fn version_comparison_ignores_fourth_segment() {
        // "1.6.0.0" treated as "1.6.0"
        assert!(!is_update_available("1.6.0.0", "1.6.0"));
    }

    #[test]
    fn version_comparison_pre_release() {
        // A stable release is newer than its own beta
        assert!(is_update_available("1.6.0-beta.1", "1.6.0"));
        assert!(!is_update_available("1.6.0", "1.6.0-beta.1"));
    }

    #[test]
    fn version_comparison_double_digit() {
        // Common regression: "1.10.0" must be > "1.9.0"
        assert!(is_update_available("1.9.0", "1.10.0"));
    }

    #[test]
    fn version_unparseable_is_not_update() {
        assert!(!is_update_available("not-a-version", "1.0.0"));
        assert!(!is_update_available("1.0.0", "not-a-version"));
    }
}