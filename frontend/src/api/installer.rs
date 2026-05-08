// installer.rs
//
// Handles installing a mod from a user-supplied zip file.
// Supports: file picker, drag-and-drop, and direct path.
//
// Detects zip structure automatically and places the mod
// into the correct Mods/ subfolder regardless of how it was packaged.

use std::path::{Path, PathBuf};
use anyhow::{bail, Context, Result};

use crate::api::mod_manager::ModManifest;

/// Result of a successful install
#[derive(Debug)]
pub struct InstallResult {
    /// The mod's folder name inside Mods/, e.g. "CJB Cheats Menu"
    pub mod_folder: String,
    /// The parsed manifest — so the UI can immediately show the new mod
    pub manifest: ModManifest,
    /// True if this replaced an existing version
    pub was_update: bool,
}

/// Install a mod from a zip file path into the given Mods/ directory.
/// Safe to call with both new mods and updates — detects existing install.
pub fn install_from_zip(zip_path: &Path, mods_dir: &Path) -> Result<InstallResult> {
    // 1. Extract to a temp staging directory
    let staging = tempfile::tempdir()
        .context("Failed to create staging directory")?;

    extract_zip(zip_path, staging.path())?;

    // 2. Find the actual mod root inside the zip (handles all 3 packaging cases)
    let mod_root = find_mod_root(staging.path())
        .context("Could not find manifest.json inside zip — may not be a valid SMAPI mod")?;

    // 3. Parse the manifest so we know the mod's name and ID
    let manifest = parse_manifest(&mod_root.join("manifest.json"))?;

    // 4. Determine destination — use the folder name from the zip if clean,
    //    otherwise fall back to the mod's Name field from manifest
    let dest_folder_name = clean_folder_name(&manifest.name);
    let dest = mods_dir.join(&dest_folder_name);

    let was_update = dest.exists();

    // 5. If updating, back up the old version first
    if was_update {
        crate::api::updater::backup_existing(&dest)?;
    }

    // 6. Atomic replace: move staging mod root → Mods/ModName/
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .with_context(|| format!("Failed to remove existing mod at {}", dest.display()))?;
    }

    // Copy rather than rename since staging may be on a different filesystem (temp dir)
    copy_dir_all(&mod_root, &dest)
        .with_context(|| format!("Failed to copy mod to {}", dest.display()))?;

    Ok(InstallResult {
        mod_folder: dest_folder_name,
        manifest,
        was_update,
    })
}

/// Walk the extracted zip to find the directory containing manifest.json.
/// Handles all three common packaging structures.
fn find_mod_root(extracted: &Path) -> Option<PathBuf> {
    // Case 3: manifest.json right at the root
    if extracted.join("manifest.json").exists() {
        return Some(extracted.to_path_buf());
    }

    // Walk one and two levels deep to cover cases 1 and 2
    for entry in std::fs::read_dir(extracted).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();

        if !path.is_dir() { continue; }

        // Case 1: Mods/ModName/manifest.json
        if path.join("manifest.json").exists() {
            return Some(path);
        }

        // Case 2: OuterWrapper/ModName/manifest.json
        for inner in std::fs::read_dir(&path).ok()? {
            let inner = inner.ok()?;
            let inner_path = inner.path();
            if inner_path.is_dir() && inner_path.join("manifest.json").exists() {
                return Some(inner_path);
            }
        }
    }

    None
}

/// Sanitize a mod name into a safe folder name.
/// "CJB Cheats Menu" → "CJB Cheats Menu" (kept as-is, just stripped of bad chars)
fn clean_folder_name(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Recursively copy a directory tree.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

/// Extract a zip archive into dest_dir (reused from updater logic).
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(zip_path)
        .with_context(|| format!("Cannot open zip: {}", zip_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)
        .context("Invalid or corrupt zip archive")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let out_path = dest_dir.join(entry.name());

        // Zip slip protection
        if !out_path.starts_with(dest_dir) {
            bail!("Zip contains unsafe path: {}", entry.name());
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }
    Ok(())
}

fn parse_manifest(path: &Path) -> Result<ModManifest> {
    let contents = std::fs::read_to_string(path)?;
    serde_json::from_str(&contents)
        .or_else(|_| json5::from_str(&contents))
        .context("Failed to parse manifest.json")
}