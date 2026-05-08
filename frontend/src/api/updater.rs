// updater.rs
//
// Downloads, backs up, and installs mod updates.
//
// Flow for a single mod update:
//   1. Get CDN download URL from nexus.rs
//   2. Stream the zip file to a temp location (with progress callback)
//   3. Back up the existing mod folder as a zip in the backups dir
//   4. Extract the new zip into a staging folder
//   5. Atomically replace the old mod folder with the staged one
//   6. Verify the new manifest.json matches the expected version
//   7. On any failure after step 3, restore from backup automatically
//
// No Nexus API calls here — the caller passes in the download URL.
// No UI — progress is reported via a callback the UI layer provides.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use anyhow::{bail, Context, Result};
use futures::StreamExt;

use crate::mods::ModManifest;
use crate::paths;

// ─── Progress reporting ───────────────────────────────────────────────────────

/// Called repeatedly during download with bytes downloaded so far and total size.
/// `total` is `None` if the server didn't send Content-Length.
pub type ProgressCallback = Box<dyn Fn(u64, Option<u64>) + Send>;

// ─── Public entry point ───────────────────────────────────────────────────────

/// Download and install a mod update.
///
/// `mod_path`       — the mod's current folder (e.g. `.../Mods/CJB Cheats Menu/`)
/// `download_url`   — CDN URL from `nexus.rs::download_url()`
/// `expected_version` — the version string we expect after install (for verification)
/// `on_progress`    — optional callback for download progress UI
///
/// On success, returns the path to the backup zip that was created.
/// On failure after backup was made, the backup is automatically restored.
pub async fn install_update(
    mod_path: &Path,
    download_url: &str,
    expected_version: &str,
    on_progress: Option<ProgressCallback>,
) -> Result<PathBuf> {
    // Step 1: Download to a temp file
    let temp_zip = download_to_temp(download_url, on_progress).await
        .context("Download failed")?;

    // Step 2: Back up the existing mod folder
    let backup_path = backup_mod(mod_path)
        .context("Backup failed — update aborted for safety")?;

    // Steps 3–5: Extract and replace (with rollback on failure)
    let result = extract_and_replace(&temp_zip, mod_path).await;

    // Clean up temp zip regardless of outcome
    let _ = std::fs::remove_file(&temp_zip);

    if let Err(e) = result {
        // Something went wrong after backup — restore automatically
        eprintln!("[updater] Install failed, restoring backup: {e}");
        if let Err(restore_err) = restore_backup(&backup_path, mod_path) {
            // This is bad — we failed to install AND failed to restore.
            // Leave the backup in place so the user can recover manually.
            bail!(
                "Install failed ({e}) and rollback also failed ({restore_err}). \
                 Backup preserved at: {}",
                backup_path.display()
            );
        }
        return Err(e.context("Update failed and was rolled back to previous version"));
    }

    // Step 6: Verify the new manifest matches expected version
    verify_installed_version(mod_path, expected_version)
        .context("Version mismatch after install")?;

    Ok(backup_path)
}

// ─── Download ─────────────────────────────────────────────────────────────────

/// Stream a file from `url` into a temporary file, reporting progress.
/// Returns the path to the temp file on success.
async fn download_to_temp(url: &str, on_progress: Option<ProgressCallback>) -> Result<PathBuf> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("Failed to build HTTP client")?;

    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to start download")?
        .error_for_status()
        .context("Download request returned an error")?;

    let total_size = response.content_length(); // None if no Content-Length header

    // Write to a temp file in the OS temp dir
    let temp_path = std::env::temp_dir().join(format!(
        "smm_download_{}.zip",
        unix_now()
    ));
    let mut file = std::fs::File::create(&temp_path)
        .with_context(|| format!("Cannot create temp file: {}", temp_path.display()))?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading download stream")?;
        file.write_all(&chunk).context("Failed to write to temp file")?;
        downloaded += chunk.len() as u64;

        if let Some(ref cb) = on_progress {
            cb(downloaded, total_size);
        }
    }

    file.flush().context("Failed to flush temp file")?;

    Ok(temp_path)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─── Backup ───────────────────────────────────────────────────────────────────

/// Compress `mod_path` into a zip archive in the app's backups directory.
/// The archive is named `{mod_folder_name}_{version}_{timestamp}.zip`.
/// Returns the path to the created archive.
fn backup_mod(mod_path: &Path) -> Result<PathBuf> {
    let folder_name = mod_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Mod folder has no valid name")?;

    // Read current version from manifest for the backup filename
    let version = read_manifest_version(mod_path).unwrap_or_else(|_| "unknown".into());
    let timestamp = unix_now();

    let backup_filename = format!("{folder_name}_{version}_{timestamp}.zip");
    let backup_path = paths::backups_dir()?.join(&backup_filename);

    zip_directory(mod_path, &backup_path)
        .with_context(|| format!("Failed to create backup zip: {}", backup_path.display()))?;

    eprintln!("[updater] Backup created: {}", backup_path.display());
    Ok(backup_path)
}

/// Compress a directory into a zip file.
fn zip_directory(src: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::create(dest)
        .with_context(|| format!("Cannot create zip: {}", dest.display()))?;

    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let prefix = src.parent().unwrap_or(src);

    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.context("Failed to walk mod directory")?;
        let path = entry.path();
        let relative = path.strip_prefix(prefix)
            .context("Failed to compute relative path")?;

        if path.is_dir() {
            zip.add_directory(relative.to_string_lossy(), options)
                .context("Failed to add directory to zip")?;
        } else {
            zip.start_file(relative.to_string_lossy(), options)
                .context("Failed to start zip file entry")?;

            let mut f = std::fs::File::open(path)
                .with_context(|| format!("Cannot read: {}", path.display()))?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).context("Failed to read file")?;
            zip.write_all(&buf).context("Failed to write to zip")?;
        }
    }

    zip.finish().context("Failed to finalize zip")?;
    Ok(())
}

// ─── Extract and replace ──────────────────────────────────────────────────────

/// Extract `zip_path` into a staging folder, then atomically replace `mod_path`.
async fn extract_and_replace(zip_path: &Path, mod_path: &Path) -> Result<()> {
    // Stage next to the real mod folder to ensure same filesystem (for atomic rename)
    let staging = mod_path.with_extension("_staging");

    // Clean up any leftover staging dir from a previous failed attempt
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .with_context(|| format!("Cannot clear staging dir: {}", staging.display()))?;
    }

    // Extract
    extract_zip(zip_path, &staging)
        .with_context(|| format!("Extraction to staging failed: {}", staging.display()))?;

    // The zip may contain a single top-level folder (common Nexus packaging).
    // If so, use that inner folder as the actual mod content.
    let actual_staging = unwrap_single_subfolder(&staging).unwrap_or(staging.clone());

    // Remove old mod folder
    std::fs::remove_dir_all(mod_path)
        .with_context(|| format!("Cannot remove old mod folder: {}", mod_path.display()))?;

    // Rename staging → mod_path (atomic on same filesystem)
    std::fs::rename(&actual_staging, mod_path)
        .with_context(|| format!("Cannot rename staging to mod path: {}", mod_path.display()))?;

    // Clean up outer staging shell if it's now empty
    let _ = std::fs::remove_dir_all(&staging);

    Ok(())
}

/// Extract a zip archive into `dest_dir`.
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;

    let file = std::fs::File::open(zip_path)
        .with_context(|| format!("Cannot open zip: {}", zip_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)
        .context("Invalid zip archive")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("Failed to read zip entry")?;
        let out_path = dest_dir.join(entry.name());

        // Zip slip protection: ensure the output path stays inside dest_dir
        if !out_path.starts_with(dest_dir) {
            bail!("Zip contains path traversal entry: {}", entry.name());
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .with_context(|| format!("Cannot create dir: {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&out_path)
                .with_context(|| format!("Cannot create: {}", out_path.display()))?;
            std::io::copy(&mut entry, &mut outfile)
                .with_context(|| format!("Cannot write: {}", out_path.display()))?;
        }
    }

    Ok(())
}

/// If `dir` contains exactly one subdirectory and nothing else, return it.
/// This handles zips packaged as `CJBCheatsMenu_1.36/CJB Cheats Menu/manifest.json`.
fn unwrap_single_subfolder(dir: &Path) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .collect();

    if entries.len() == 1 && entries[0].path().is_dir() {
        Some(entries[0].path())
    } else {
        None
    }
}

// ─── Restore ──────────────────────────────────────────────────────────────────

/// Restore a mod folder from a backup zip.
/// Removes whatever is currently at `mod_path` and extracts the backup there.
fn restore_backup(backup_path: &Path, mod_path: &Path) -> Result<()> {
    eprintln!("[updater] Restoring from backup: {}", backup_path.display());

    // Remove whatever partial state may have been left
    if mod_path.exists() {
        std::fs::remove_dir_all(mod_path)
            .with_context(|| format!("Cannot clear mod path for restore: {}", mod_path.display()))?;
    }

    let staging = mod_path.with_extension("_restore");
    extract_zip(backup_path, &staging)?;

    let actual = unwrap_single_subfolder(&staging).unwrap_or(staging.clone());
    std::fs::rename(&actual, mod_path)
        .context("Failed to rename restored mod into place")?;
    let _ = std::fs::remove_dir_all(&staging);

    eprintln!("[updater] Restore complete: {}", mod_path.display());
    Ok(())
}

// ─── Verification ─────────────────────────────────────────────────────────────

/// Read the version string from `mod_path/manifest.json`.
fn read_manifest_version(mod_path: &Path) -> Result<String> {
    let manifest_path = mod_path.join("manifest.json");
    let contents = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Cannot read manifest: {}", manifest_path.display()))?;

    let manifest: ModManifest = serde_json::from_str(&contents)
        .or_else(|_| json5::from_str(&contents))
        .context("Cannot parse manifest")?;

    Ok(manifest.version)
}

/// After install, confirm the manifest version matches what we expected.
fn verify_installed_version(mod_path: &Path, expected: &str) -> Result<()> {
    let installed = read_manifest_version(mod_path)?;

    // Normalize both for comparison (strip leading 'v', whitespace)
    let installed_clean = installed.trim().trim_start_matches('v');
    let expected_clean  = expected.trim().trim_start_matches('v');

    if installed_clean != expected_clean {
        bail!(
            "Version mismatch after install: expected {expected_clean}, got {installed_clean}. \
             The backup has been preserved."
        );
    }

    Ok(())
}

// ─── Backup management ────────────────────────────────────────────────────────

/// List all backup archives, sorted newest first.
pub fn list_backups() -> Result<Vec<PathBuf>> {
    let dir = paths::backups_dir()?;

    let mut backups: Vec<PathBuf> = std::fs::read_dir(&dir)
        .with_context(|| format!("Cannot read backups dir: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("zip"))
        .collect();

    // Sort by modification time, newest first
    backups.sort_by(|a, b| {
        let mt_a = a.metadata().and_then(|m| m.modified()).ok();
        let mt_b = b.metadata().and_then(|m| m.modified()).ok();
        mt_b.cmp(&mt_a)
    });

    Ok(backups)
}

/// Delete backup archives older than `keep_days` days, keeping at least `keep_min`.
pub fn prune_backups(keep_days: u64, keep_min: usize) -> Result<usize> {
    let backups = list_backups()?;
    let cutoff = unix_now().saturating_sub(keep_days * 86_400);
    let mut deleted = 0;

    for (i, path) in backups.iter().enumerate() {
        if i < keep_min {
            continue; // Always keep the most recent N backups
        }

        let mtime = path
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if mtime < cutoff {
            std::fs::remove_file(path)
                .with_context(|| format!("Failed to delete old backup: {}", path.display()))?;
            deleted += 1;
        }
    }

    Ok(deleted)
}