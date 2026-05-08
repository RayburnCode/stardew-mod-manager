// paths.rs
//
// Single source of truth for every filesystem path this app touches.
// All other modules import from here — nothing calls `dirs` directly.
//
// Two categories of paths:
//   1. Stardew paths  – where the game and its mods live
//   2. App data paths – where WE store config, cache, and backups

use std::path::PathBuf;
use anyhow::{Context, Result};

// ─── Stardew paths ────────────────────────────────────────────────────────────

/// Returns the default Stardew Valley `Mods/` folder for the current OS.
///
/// This is the Steam default location. Users who installed elsewhere
/// (GOG, custom Steam library, etc.) will need to override via `AppConfig`.
///
/// Returns `None` if the home directory can't be determined — unlikely
/// in practice but possible in sandboxed or CI environments.
pub fn default_mods_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    let path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Steam/steamapps/common/Stardew Valley/Mods")
    } else if cfg!(target_os = "windows") {
        // On Windows, Steam typically lives under Program Files (x86).
        // `dirs::home_dir()` gives us e.g. C:\Users\Alice, so we build
        // the Steam path relative to the drive root instead.
        //
        // We grab the prefix (e.g. "C:") from home and append from there.
        let prefix = home
            .components()
            .next()
            .map(|c| PathBuf::from(c.as_os_str()))
            .unwrap_or_else(|| PathBuf::from("C:"));

        prefix.join("Program Files (x86)/Steam/steamapps/common/Stardew Valley/Mods")
    } else {
        // Linux (including Steam Deck)
        home.join(".steam/steam/steamapps/common/Stardew Valley/Mods")
    };

    Some(path)
}

/// Validates that a given path actually looks like a Stardew `Mods/` folder.
///
/// We can't be 100% certain, but if the directory exists and contains at
/// least one subfolder with a `manifest.json`, it's almost certainly right.
/// Used to warn users when their override path looks wrong.
pub fn looks_like_mods_folder(path: &std::path::Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    // Check for at least one mod subfolder containing manifest.json
    std::fs::read_dir(path)
        .ok()
        .and_then(|mut entries| {
            entries.find(|e| {
                e.as_ref()
                    .map(|e| e.path().join("manifest.json").exists())
                    .unwrap_or(false)
            })
        })
        .is_some()
}

// ─── App data paths ───────────────────────────────────────────────────────────
//
// `dirs` resolves these to the correct OS-standard location:
//
//   config_dir():
//     macOS   → ~/Library/Application Support/
//     Windows → C:\Users\Alice\AppData\Roaming\
//     Linux   → ~/.config/
//
//   cache_dir():
//     macOS   → ~/Library/Caches/
//     Windows → C:\Users\Alice\AppData\Local\Temp\
//     Linux   → ~/.cache/
//
//   data_local_dir():
//     macOS   → ~/Library/Application Support/
//     Windows → C:\Users\Alice\AppData\Local\
//     Linux   → ~/.local/share/
//
// We append "stardew-mod-manager" to each so our files are namespaced.

const APP_NAME: &str = "stardew-mod-manager";

/// `~/.config/stardew-mod-manager/` (or OS equivalent)
/// Stores: config.json (user settings, API key reference, path overrides)
pub fn config_dir() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("Could not determine config directory for this OS")?
        .join(APP_NAME);

    // Create the directory if it doesn't exist yet
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create config dir: {}", dir.display()))?;

    Ok(dir)
}

/// Full path to the config file itself.
pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

/// `~/.cache/stardew-mod-manager/` (or OS equivalent)
/// Stores: nexus_cache.json (API responses with timestamps for TTL checks)
pub fn cache_dir() -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .context("Could not determine cache directory for this OS")?
        .join(APP_NAME);

    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create cache dir: {}", dir.display()))?;

    Ok(dir)
}

/// Full path to the Nexus API response cache file.
pub fn nexus_cache_file() -> Result<PathBuf> {
    Ok(cache_dir()?.join("nexus_cache.json"))
}

/// `~/.local/share/stardew-mod-manager/backups/` (or OS equivalent)
/// Stores: one zip per mod version before it's overwritten by an update.
/// e.g. backups/CJB_Cheats_Menu_1.35.0.zip
pub fn backups_dir() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .context("Could not determine local data directory for this OS")?
        .join(APP_NAME)
        .join("backups");

    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create backups dir: {}", dir.display()))?;

    Ok(dir)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mods_path_returns_some() {
        // As long as we're not in a completely stripped environment,
        // home_dir should resolve and give us a path.
        let path = default_mods_path();
        assert!(path.is_some(), "Expected a default mods path");
    }

    #[test]
    fn config_dir_creates_directory() {
        let dir = config_dir().expect("config_dir should succeed");
        assert!(dir.exists(), "config_dir should create the directory");
    }

    #[test]
    fn looks_like_mods_folder_rejects_nonexistent() {
        let fake = PathBuf::from("/this/path/does/not/exist");
        assert!(!looks_like_mods_folder(&fake));
    }
}