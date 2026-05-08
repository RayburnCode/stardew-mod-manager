// config.rs
//
// Owns the user's persisted settings: their Mods/ path override,
// Nexus API key state, and UI preferences.
//
// Serialized to JSON at the path returned by `paths::config_file()`.
//
// Usage:
//   let config = AppConfig::load()?;       // load or create default
//   config.mods_path_override = Some(...); // mutate
//   config.save()?;                        // persist

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

use crate::api::paths;

// ─── AppConfig ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// User-specified path to the Stardew `Mods/` folder.
    /// `None` means "use the OS default" from `paths::default_mods_path()`.
    pub mods_path_override: Option<PathBuf>,

    /// How long (in seconds) to trust a cached Nexus API response
    /// before re-fetching. Default: 3600 (1 hour).
    pub cache_ttl_seconds: u64,

    /// Whether to show mods with unknown update sources in the UI.
    /// Some players prefer to hide them to reduce noise.
    pub show_unknown_source_mods: bool,

    /// This flag tracks whether an API key has been saved.
    /// The key value itself lives in a separate local file.
    pub nexus_api_key_saved: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mods_path_override: None,
            cache_ttl_seconds: 3600,
            show_unknown_source_mods: true,
            nexus_api_key_saved: false,
        }
    }
}

impl AppConfig {
    /// Load config from disk, or return `Default` if the file doesn't exist yet.
    ///
    /// Returns an error only if the file exists but can't be read or parsed —
    /// a missing file is treated as "first run" and silently returns defaults.
    pub fn load() -> Result<Self> {
        let path = paths::config_file()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        serde_json::from_str(&contents)
            .with_context(|| format!("Config file is invalid JSON: {}", path.display()))
    }

    /// Persist the current config to disk.
    ///
    /// Writes atomically: serializes to a temp file first, then renames.
    /// This prevents a half-written config if the app crashes mid-save.
    pub fn save(&self) -> Result<()> {
        let path = paths::config_file()?;

        // Write to a sibling temp file first
        let tmp_path = path.with_extension("json.tmp");

        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;

        std::fs::write(&tmp_path, &json)
            .with_context(|| format!("Failed to write temp config: {}", tmp_path.display()))?;

        // Atomic rename — on the same filesystem this is a metadata-only op
        std::fs::rename(&tmp_path, &path)
            .with_context(|| format!("Failed to finalize config save: {}", path.display()))?;

        Ok(())
    }

    /// Resolve the actual mods path to use — either the user's override
    /// or the OS default. Returns `None` only if neither is available
    /// (extremely rare: no home directory and no override set).
    pub fn resolved_mods_path(&self) -> Option<PathBuf> {
        self.mods_path_override
            .clone()
            .or_else(paths::default_mods_path)
    }
}

// ─── Nexus API key (local file) ───────────────────────────────────────────────

/// Save the Nexus API key to a local config file and mark it saved in config.
pub fn save_api_key(config: &mut AppConfig, key: &str) -> Result<()> {
    let path = paths::nexus_api_key_file()?;
    let trimmed = key.trim();

    std::fs::write(&path, trimmed)
        .with_context(|| format!("Failed to save API key: {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

    config.nexus_api_key_saved = true;
    config.save()?;

    Ok(())
}

/// Retrieve the Nexus API key from local config storage.
/// Returns `None` if no key has been saved yet.
pub fn load_api_key() -> Result<Option<String>> {
    let path = paths::nexus_api_key_file()?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read API key: {}", path.display()))?;
    let key = raw.trim().to_string();

    if key.is_empty() {
        return Ok(None);
    }

    Ok(Some(key))
}

/// Delete the stored API key and update config.
pub fn delete_api_key(config: &mut AppConfig) -> Result<()> {
    let path = paths::nexus_api_key_file()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to remove API key: {}", path.display()))?;
    }

    config.nexus_api_key_saved = false;
    config.save()?;

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let config = AppConfig::default();
        assert!(config.mods_path_override.is_none());
        assert_eq!(config.cache_ttl_seconds, 3600);
        assert!(config.show_unknown_source_mods);
        assert!(!config.nexus_api_key_saved);
    }

    #[test]
    fn roundtrip_serialize() {
        let mut config = AppConfig::default();
        config.mods_path_override = Some(PathBuf::from("/custom/Mods"));
        config.cache_ttl_seconds = 7200;

        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.mods_path_override, config.mods_path_override);
        assert_eq!(restored.cache_ttl_seconds, 7200);
    }

    #[test]
    fn resolved_mods_path_prefers_override() {
        let mut config = AppConfig::default();
        let custom = PathBuf::from("/my/custom/Mods");
        config.mods_path_override = Some(custom.clone());

        assert_eq!(config.resolved_mods_path(), Some(custom));
    }

    #[test]
    fn resolved_mods_path_falls_back_to_default() {
        let config = AppConfig::default();
        // Should equal whatever paths::default_mods_path() returns
        assert_eq!(config.resolved_mods_path(), paths::default_mods_path());
    }
}