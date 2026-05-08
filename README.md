<!-- @format -->

# Stardew Mod Manager

A lightweight, cross-platform desktop mod manager for **Stardew Valley**.  
See what's installed, what's outdated, and update mods — without leaving the app.

> Built with Rust + [Dioxus](https://dioxuslabs.com) · SMAPI-compatible · Not a SMAPI replacement

---

## Screenshots

<!-- After taking screenshots, drop them in docs/screenshots/ and swap these paths -->

<!-- ![Mod list view](docs/screenshots/mod-list.png) -->
<!-- ![Settings view](docs/screenshots/settings.png) -->
<!-- ![Update in progress](docs/screenshots/update.png) -->

> Screenshots coming soon.

---

## Features

- **Mod discovery** — scans your `Mods/` folder and reads every `manifest.json` automatically
- **Update checking** — queries both the [SMAPI API](https://smapi.io/api/v3.0/mods) and [Nexus Mods API](https://api.nexusmods.com/v1/) concurrently and merges results
- **One-click updates** — downloads, backs up, and replaces mod folders atomically (Nexus Premium)
- **Browser fallback** — opens the correct Nexus mod page for free-tier users
- **Auto backup** — keeps a versioned `.zip` of every mod before it's overwritten
- **Local API key storage** — your Nexus API key is saved locally, no OS keychain prompts
- **Cross-platform** — macOS, Windows, and Linux

---

## Requirements

- [Rust](https://rustup.rs) stable toolchain
- [Dioxus CLI](https://dioxuslabs.com/learn/0.7/getting_started) (`cargo install dioxus-cli`)
- A [Nexus Mods](https://www.nexusmods.com) account and personal API key (free tier works for update checks; Premium required for direct downloads)

---

## Getting Started

```bash
git clone https://github.com/RayburnCode/stardew-mod-manager
cd stardew-mod-manager/frontend
dx serve --platform desktop
```

On first launch:

1. Go to **Settings → Nexus Mods API Key** and paste your key
2. Click **Scan Mods Folder** to discover installed mods
3. Click **Check for Updates** to compare against SMAPI and Nexus

---

## Project Structure

```
frontend/
  src/
    main.rs              # Entry point
    routes.rs            # Route enum
    api/
      config.rs          # Settings + API key (local file storage)
      mod_manager.rs     # Mod discovery and version comparison
      nexus.rs           # Nexus Mods API client
      smapi-api.rs       # SMAPI compatibility API client
      updater.rs         # Download, backup, and install logic
      paths.rs           # All filesystem paths in one place
      app_state.rs       # Global Dioxus app state
    views/
      mod_list.rs        # Main mod table + toolbar
      settings.rs        # Settings screen
      installer.rs       # Manual install flow
    components/
      navbar.rs
      layout.rs
      footer.rs
```

---

## API Key Storage

The Nexus API key is stored in a local config file at:

| Platform | Path                                                                  |
| -------- | --------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/stardew-mod-manager/nexus_api_key.txt` |
| Windows  | `%APPDATA%\stardew-mod-manager\nexus_api_key.txt`                     |
| Linux    | `~/.config/stardew-mod-manager/nexus_api_key.txt`                     |

The file is created with `600` permissions (owner read/write only) on Unix systems.

---

## Contributing

This project is in active development. See [SCOPE.md](SCOPE.md) for the full feature plan and current implementation backlog.

---

## License

[MIT](LICENSE)
