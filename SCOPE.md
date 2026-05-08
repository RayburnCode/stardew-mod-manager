<!-- @format -->

# agents.md — Stardew Mod Manager (Dioxus + Rust)

A lightweight, cross-platform mod manager for Stardew Valley that reads installed mods,
checks for updates via the Nexus Mods API, and supports one-click installs. SMAPI-compatible,
not a replacement.

---

## Project Overview

**Stack:** Rust + Dioxus (desktop target) + Nexus Mods API  
**Platforms:** macOS, Windows, Linux  
**Goal:** Give players a clean GUI to see what's installed, what's outdated, and update with one click.

---

## Phase 2 — Mod Discovery

**Goal:** Detect the Stardew Valley install and read all installed mods from the `Mods/` folder.

### Key Knowledge

SMAPI mods each live in their own subfolder under `Mods/` and contain a `manifest.json` with this structure:

```json
{
  "Name": "SMAPI",
  "Author": "Pathoschild",
  "Version": "4.0.0",
  "Description": "...",
  "UniqueID": "Pathoschild.SMAPI",
  "UpdateKeys": ["Nexus:2400"]
}
```

The `UpdateKeys` field is critical — it tells you where to check for updates (e.g., `Nexus:2400` = Nexus mod ID 2400).

### Steps

1. Create a `ModManifest` struct that deserializes `manifest.json` fields
2. Write a `discover_mods(mods_path: &Path) -> Vec<ModManifest>` function that:
   - Reads all subdirectories of the `Mods/` folder
   - Attempts to parse `manifest.json` in each
   - Skips folders with no manifest gracefully (logs a warning)
3. Implement cross-platform path detection in a `find_stardew_path()` function:

   | Platform | Default Path                                                               |
   | -------- | -------------------------------------------------------------------------- |
   | macOS    | `~/Library/Application Support/Steam/steamapps/common/Stardew Valley/Mods` |
   | Windows  | `C:\Program Files (x86)\Steam\steamapps\common\Stardew Valley\Mods`        |
   | Linux    | `~/.steam/steam/steamapps/common/Stardew Valley/Mods`                      |

4. Allow user to manually override the path (store in a local config file)
5. Display discovered mods in the UI as a simple list (name, author, version)

### Acceptance Criteria

- All mods in the `Mods/` folder appear in the UI
- Malformed or missing manifests don't crash the app
- Manual path override persists across app restarts

---

## Phase 3 — Nexus Mods API Integration

**Goal:** For each mod with a `Nexus:XXXX` update key, fetch the latest version from the Nexus API.

### API Details

- Base URL: `https://api.nexusmods.com/v1/`
- Relevant endpoint: `GET /games/stardewvalley/mods/{mod_id}.json`
- Auth: Requires a personal API key sent as `apikey` header
- Rate limit: 100 requests/hour (daily member), 2500/day — handle gracefully
- Docs: https://app.swaggerhub.com/apis-list/NexusMods/nexus-mods_public_api/1.0

### Steps

1. Add a settings screen where the user pastes their Nexus API key
2. Store the API key securely using OS keychain (use the `keyring` crate)
3. Create a `NexusClient` struct wrapping `reqwest::Client` with the API key header set
4. Write `fetch_mod_info(client: &NexusClient, mod_id: u32) -> Result<NexusModInfo>`
5. Parse the response into a `NexusModInfo` struct (capture: `version`, `name`, `updated_timestamp`)
6. Run all API calls concurrently with `tokio::join!` or `futures::join_all` — don't fetch serially
7. Cache results locally (JSON file) with a TTL of ~1 hour to avoid hammering the rate limit
8. Show a loading state in the UI while fetching

### Acceptance Criteria

- Latest version is fetched and displayed next to each mod's installed version
- API key is not stored in plaintext
- Rate limit errors (HTTP 429) are caught and shown as a friendly message
- Mods without a `Nexus:` update key are gracefully skipped (show "no update source")

---

## Phase 4 — Update Detection & Status Display

**Goal:** Compare installed vs. latest versions and surface outdated mods clearly.

### Steps

1. Implement semantic version comparison — parse versions like `1.2.3` and compare correctly
   - Use the `semver` crate; handle non-standard versions like `1.2.3-beta` gracefully
2. Add a `ModStatus` enum:
   ```rust
   enum ModStatus {
       UpToDate,
       UpdateAvailable { latest: String },
       Unknown,        // No update key or API fetch failed
       Incompatible,   // Future: SMAPI compatibility check
   }
   ```
3. Display status in the UI with visual indicators:
   - ✅ Up to date
   - 🔼 Update available (highlight with version diff)
   - ❓ Unknown source
4. Add a filter/sort bar: "Show all", "Updates only", sort by name/status
5. Show a summary badge: e.g., "3 updates available"

### Acceptance Criteria

- Version comparison is correct (1.10.0 > 1.9.0)
- UI clearly distinguishes up-to-date vs. outdated mods
- Filter and sort work without re-fetching from API

---

## Phase 5 — One-Click Update Downloads

**Goal:** Let users download and install the latest version of a mod in one click.

### API Details

The Nexus files endpoint provides download links:

- `GET /games/stardewvalley/mods/{mod_id}/files.json` — lists all file versions
- `GET /games/stardewvalley/mods/{mod_id}/files/{file_id}/download_link.json` — gets a CDN URL
- Note: Free Nexus accounts can only generate download links via the site; **NXM protocol or Premium API key** is needed for direct programmatic download. Consider linking to the mod page as a fallback for free users.

### Steps

1. Detect whether the user's API key is Premium (the API returns this in the user endpoint)
2. **For Premium users:**
   - Fetch the latest main file ID from the files endpoint
   - Get a download URL
   - Stream the download to a temp file with a progress bar
   - Unzip to a temp location, then replace the existing mod folder atomically
   - Back up the old version to a `_backups/` folder before overwriting
3. **For free users:**
   - Show an "Open on Nexus" button that launches the browser to the mod page
4. After update, re-read the mod's `manifest.json` to confirm the new version
5. Rollback: if the new `manifest.json` version still doesn't match, restore from backup

### Acceptance Criteria

- Premium: mod is downloaded, extracted, and installed without manual steps
- Free: browser opens to the correct Nexus mod page
- Old version is backed up before overwriting
- Installation failure restores the previous version automatically

---

## Phase 6 — SMAPI Compatibility Check (Optional Enhancement)

**Goal:** Warn users if a mod is known to be incompatible with their SMAPI version.

### Steps

1. Detect installed SMAPI version from its `manifest.json` in the Mods folder
2. Fetch the SMAPI compatibility list from:
   `https://smapi.io/mods` (parsed) or use the unofficial JSON:  
   `https://raw.githubusercontent.com/Pathoschild/SMAPI/develop/docs/release-notes.md`
   — Better: use `https://smapi.io/api/v3.0/mods` if available
3. Cross-reference each installed mod's `UniqueID` against the compatibility data
4. Add `Incompatible` and `Broken` states to `ModStatus`
5. Surface these warnings prominently — don't bury them

### Acceptance Criteria

- Mods flagged as broken by SMAPI show a clear warning
- SMAPI version detection works across platforms

---

## Data Flow Summary

```
App Start
  └─ find_stardew_path()
       └─ discover_mods() → Vec<ModManifest>
            └─ for each mod with UpdateKeys
                 └─ NexusClient::fetch_mod_info() [concurrent]
                      └─ compare_versions()
                           └─ ModStatus → UI render

On "Update" click
  └─ fetch_download_url()
       └─ stream_download() → temp file
            └─ backup_existing_mod()
                 └─ extract_and_replace()
                      └─ verify_manifest()
```

---

## Key Crates Reference

| Crate                       | Purpose                                       |
| --------------------------- | --------------------------------------------- |
| `dioxus` + `dioxus-desktop` | UI framework                                  |
| `tokio`                     | Async runtime                                 |
| `reqwest`                   | HTTP client for Nexus API                     |
| `serde` + `serde_json`      | Deserialize `manifest.json` and API responses |
| `semver`                    | Version parsing and comparison                |
| `dirs`                      | Cross-platform home/app directory paths       |
| `keyring`                   | Secure OS keychain storage for API key        |
| `zip`                       | Extract downloaded mod archives               |
| `futures`                   | `join_all` for concurrent API fetches         |

---

## Notes & Gotchas

- **Nexus download links require Premium** for programmatic use. Design the free-user fallback early — don't treat it as an afterthought.
- **`UpdateKeys` is optional** — many mods on Nexus don't include it. Show these as "unknown source" and don't error.
- **Atomic folder replacement** on Windows is tricky due to file locking. Use a rename strategy: extract to `ModName_new/`, delete `ModName/`, rename `ModName_new/` → `ModName/`.
- **SMAPI itself** lives in the `Mods/` folder but isn't a regular mod. Skip it or display it separately.
- **Version strings aren't always semver** — some mods use `1.0` or `1.0.0.0`. Be defensive in parsing.
- **Rate limiting** — if a user has 100+ mods, you may hit the 100 req/hour limit. Batch requests where possible and show remaining quota.
