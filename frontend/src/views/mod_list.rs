// ui/mod_list.rs
//
// The main view: a table of installed mods with their status,
// a toolbar for scanning/refreshing, and per-row update actions.

use dioxus::prelude::*;
use crate::api::app_state::{AppState, Screen};
use crate::api::mod_manager::{InstalledMod, ModStatus};
//use crate::api::config;
use crate::api::nexus::NexusClient;

// ─── ModList view ─────────────────────────────────────────────────────────────

#[component]
pub fn ModList() -> Element {
    let state: AppState = use_context();
    let mods    = state.mods.read();
    let loading = *state.loading.read();
    let error   = state.error.read().clone();

    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                height: 100%;
                overflow: hidden;
            ",

            // ── Toolbar ───────────────────────────────────────────────────────
            Toolbar {}

            // ── Error banner ──────────────────────────────────────────────────
            if let Some(err) = error {
                ErrorBanner { message: err }
            }

            // ── Content area ──────────────────────────────────────────────────
            div {
                style: "flex: 1; overflow-y: auto; padding: 0 20px 20px;",

                if loading {
                    LoadingState {}
                } else if mods.is_empty() {
                    EmptyState {}
                } else {
                    // Summary row
                    {
                        let total    = mods.len();
                        let updates  = mods.iter().filter(|m| matches!(m.status, ModStatus::UpdateAvailable { .. })).count();
                        rsx! { SummaryBar { total, updates } }
                    }

                    // Column headers
                    ModTableHeader {}

                    // Mod rows
                    for installed_mod in mods.iter() {
                        ModRow { mod_data: installed_mod.clone() }
                    }
                }
            }
        }
    }
}

// ─── Toolbar ──────────────────────────────────────────────────────────────────

#[component]
fn Toolbar() -> Element {
    let mut state: AppState = use_context();

    // Scan mods and then fetch Nexus data
    let on_scan = move |_| {
        let mut state = state.clone();
        spawn(async move {
            *state.error.write() = None;
            *state.loading.write() = true;
            *state.mods.write() = vec![];

            // 1. Resolve mods path from config
            let config = state.config.read().clone();
            let Some(mods_path) = config.resolved_mods_path() else {
                *state.error.write() = Some(
                    "No Stardew Valley Mods folder found. Set a path in Settings.".into()
                );
                *state.loading.write() = false;
                return;
            };

            // 2. Discover installed mods
            let discovered = match crate::api::mod_manager::discover_mods(&mods_path) {
                Ok(m) => m,
                Err(e) => {
                    *state.error.write() = Some(format!("Scan failed: {e}"));
                    *state.loading.write() = false;
                    return;
                }
            };

            *state.mods.write() = discovered.clone();

            // 3. Fetch Nexus data if API key is set
            if !config.nexus_api_key_saved {
                *state.loading.write() = false;
                return;
            }

            let api_key = match crate::api::config::load_api_key() {
                Ok(Some(k)) => k,
                Ok(None) => {
                    *state.loading.write() = false;
                    return;
                }
                Err(e) => {
                    *state.error.write() = Some(format!("Keychain error: {e}"));
                    *state.loading.write() = false;
                    return;
                }
            };

            let mut client = match NexusClient::new(&api_key, config.cache_ttl_seconds) {
                Ok(c) => c,
                Err(e) => {
                    *state.error.write() = Some(format!("API client error: {e}"));
                    *state.loading.write() = false;
                    return;
                }
            };

            // Collect Nexus IDs for mods that have them
            let nexus_ids: Vec<u32> = discovered
                .iter()
                .filter_map(|m| m.manifest.nexus_id())
                .collect();

            let results = client.fetch_many(&nexus_ids).await;

            // Apply statuses back to the mod list
            let updated_mods: Vec<InstalledMod> = discovered
                .into_iter()
                .map(|mut m| {
                    if let Some(nexus_id) = m.manifest.nexus_id() {
                        m.status = match results.get(&nexus_id) {
                            Some(Ok(info)) => {
                                if crate::api::mod_manager::is_update_available(&m.manifest.version, &info.version) {
                                    ModStatus::UpdateAvailable { latest: info.version.clone() }
                                } else {
                                    ModStatus::UpToDate
                                }
                            }
                            Some(Err(e)) => ModStatus::FetchFailed { reason: e.to_string() },
                            None => ModStatus::Unknown,
                        };
                    }
                    m
                })
                .collect();

            *state.mods.write() = updated_mods;
            *state.loading.write() = false;
        });
    };

    rsx! {
        div {
            style: "
                display: flex;
                align-items: center;
                gap: 10px;
                padding: 14px 20px;
                border-bottom: 1px solid #1e2130;
                flex-shrink: 0;
            ",

            // Scan button
            button {
                onclick: on_scan,
                style: "
                    background: #7ec8a4;
                    color: #0c0e14;
                    border: none;
                    padding: 7px 16px;
                    border-radius: 6px;
                    font-size: 13px;
                    font-family: inherit;
                    font-weight: 600;
                    cursor: pointer;
                    letter-spacing: 0.02em;
                ",
                "Scan Mods"
            }

            // Spacer
            div { style: "flex: 1;" }

            // Filter pill: updates only (future enhancement placeholder)
            span {
                style: "font-size: 12px; color: #4b5563;",
                "Auto-refreshes from cache"
            }
        }
    }
}

// ─── Summary bar ──────────────────────────────────────────────────────────────

#[component]
fn SummaryBar(total: usize, updates: usize) -> Element {
    rsx! {
        div {
            style: "
                display: flex;
                align-items: center;
                gap: 16px;
                padding: 14px 0 10px;
                font-size: 12px;
                color: #6b7280;
            ",
            span { "{total} mods installed" }
            if updates > 0 {
                span {
                    style: "
                        background: #1a2d1f;
                        color: #7ec8a4;
                        padding: 2px 10px;
                        border-radius: 99px;
                        font-weight: 600;
                    ",
                    "{updates} update{if updates == 1 { \"\" } else { \"s\" }} available"
                }
            }
        }
    }
}

// ─── Table header ─────────────────────────────────────────────────────────────

#[component]
fn ModTableHeader() -> Element {
    rsx! {
        div {
            style: "
                display: grid;
                grid-template-columns: 1fr 120px 120px 140px;
                gap: 12px;
                padding: 6px 12px;
                font-size: 11px;
                font-weight: 600;
                letter-spacing: 0.08em;
                text-transform: uppercase;
                color: #4b5563;
                border-bottom: 1px solid #1e2130;
                margin-bottom: 4px;
            ",
            span { "Name" }
            span { "Installed" }
            span { "Latest" }
            span { "Status" }
        }
    }
}

// ─── Mod row ──────────────────────────────────────────────────────────────────

#[component]
fn ModRow(mod_data: InstalledMod) -> Element {
    let mut state: AppState = use_context();
    let name    = mod_data.manifest.name.clone();
    let author  = mod_data.manifest.author.clone();
    let version = mod_data.manifest.version.clone();

    let (latest_text, status_el) = match &mod_data.status {
        ModStatus::UpToDate => (
            version.clone(),
            rsx! {
                StatusPill { color: "#1a2d1f", text_color: "#7ec8a4", label: "Up to date" }
            },
        ),
        ModStatus::UpdateAvailable { latest } => {
            let latest = latest.clone();
            let mod_path = mod_data.path.clone();
            let dl_latest = latest.clone();

            let on_update = move |_| {
                let mut state = state.clone();
                let mod_path = mod_path.clone();
                let expected = dl_latest.clone();

                spawn(async move {
                    *state.error.write() = None;

                    // Get the API key and build client
                    let config = state.config.read().clone();
                    let api_key = match crate::config::load_api_key() {
                        Ok(Some(k)) => k,
                        _ => {
                            *state.error.write() = Some("No API key — add one in Settings.".into());
                            return;
                        }
                    };

                    let mut client = match NexusClient::new(&api_key, config.cache_ttl_seconds) {
                        Ok(c) => c,
                        Err(e) => {
                            *state.error.write() = Some(format!("Client error: {e}"));
                            return;
                        }
                    };

                    // Get Nexus ID from the mod's manifest
                    let nexus_id = match crate::mods::discover_mods(&mod_path.parent().unwrap_or(&mod_path))
                        .ok()
                        .and_then(|mods| mods.into_iter().find(|m| m.path == mod_path))
                        .and_then(|m| m.manifest.nexus_id())
                    {
                        Some(id) => id,
                        None => {
                            *state.error.write() = Some("Could not determine Nexus mod ID.".into());
                            return;
                        }
                    };

                    // Get the latest file and download URL
                    let file = match client.latest_main_file(nexus_id).await {
                        Ok(f) => f,
                        Err(e) => {
                            *state.error.write() = Some(format!("File lookup failed: {e}"));
                            return;
                        }
                    };

                    let url = match client.download_url(nexus_id, file.file_id).await {
                        Ok(u) => u,
                        Err(e) => {
                            // Likely not Premium — surface the message
                            *state.error.write() = Some(e.to_string());
                            return;
                        }
                    };

                    // Run the update
                    match crate::updater::install_update(&mod_path, &url, &expected, None).await {
                        Ok(_) => {
                            // Re-scan to reflect new version
                            // (simplest approach: just mark the mod as up-to-date in place)
                            let mut mods = state.mods.write();
                            if let Some(m) = mods.iter_mut().find(|m| m.path == mod_path) {
                                m.manifest.version = expected.clone();
                                m.status = ModStatus::UpToDate;
                            }
                        }
                        Err(e) => {
                            *state.error.write() = Some(format!("Update failed: {e}"));
                        }
                    }
                });
            };

            (
                latest.clone(),
                rsx! {
                    div {
                        style: "display: flex; align-items: center; gap: 8px;",
                        StatusPill { color: "#2d1f0a", text_color: "#f0a050", label: "Update" }
                        button {
                            onclick: on_update,
                            style: "
                                background: #f0a050;
                                color: #0c0e14;
                                border: none;
                                padding: 3px 10px;
                                border-radius: 5px;
                                font-size: 11px;
                                font-family: inherit;
                                font-weight: 700;
                                cursor: pointer;
                                letter-spacing: 0.04em;
                            ",
                            "↑ Install"
                        }
                    }
                },
            )
        }
        ModStatus::FetchFailed { .. } => (
            "—".into(),
            rsx! { StatusPill { color: "#2d1519", text_color: "#e06060", label: "Fetch error" } },
        ),
        ModStatus::Unknown => (
            "—".into(),
            rsx! { StatusPill { color: "#1a1c24", text_color: "#4b5563", label: "No source" } },
        ),
    };

    rsx! {
        div {
            style: "
                display: grid;
                grid-template-columns: 1fr 120px 120px 140px;
                gap: 12px;
                align-items: center;
                padding: 10px 12px;
                border-radius: 7px;
                margin-bottom: 2px;
                background: #0f1117;
                border: 1px solid #1e2130;
                transition: border-color 0.15s;
            ",

            // Name + author
            div {
                div {
                    style: "font-size: 13px; color: #e8e6df; font-weight: 500;",
                    "{name}"
                }
                div {
                    style: "font-size: 11px; color: #4b5563; margin-top: 1px;",
                    "{author}"
                }
            }

            // Installed version
            span {
                style: "font-size: 12px; color: #6b7280; font-variant-numeric: tabular-nums;",
                "{version}"
            }

            // Latest version
            span {
                style: "font-size: 12px; color: #6b7280; font-variant-numeric: tabular-nums;",
                "{latest_text}"
            }

            // Status / action
            { status_el }
        }
    }
}

// ─── Status pill ──────────────────────────────────────────────────────────────

#[component]
fn StatusPill(color: &'static str, text_color: &'static str, label: &'static str) -> Element {
    rsx! {
        span {
            style: "
                display: inline-block;
                background: {color};
                color: {text_color};
                font-size: 11px;
                font-weight: 600;
                padding: 3px 10px;
                border-radius: 99px;
                letter-spacing: 0.04em;
            ",
            "{label}"
        }
    }
}

// ─── Loading state ────────────────────────────────────────────────────────────

#[component]
fn LoadingState() -> Element {
    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                align-items: center;
                justify-content: center;
                height: 300px;
                gap: 12px;
                color: #4b5563;
                font-size: 13px;
            ",
            span { style: "font-size: 24px;", "⟳" }
            span { "Scanning mods…" }
        }
    }
}

// ─── Empty state ──────────────────────────────────────────────────────────────

#[component]
fn EmptyState() -> Element {
    rsx! {
        div {
            style: "
                display: flex;
                flex-direction: column;
                align-items: center;
                justify-content: center;
                height: 300px;
                gap: 10px;
                color: #4b5563;
                text-align: center;
            ",
            div {
                style: "font-size: 32px; opacity: 0.4;",
                "⬡"
            }
            div {
                style: "font-size: 14px; color: #6b7280;",
                "No mods scanned yet"
            }
            div {
                style: "font-size: 12px; color: #374151;",
                "Press Scan Mods to find your installed mods"
            }
        }
    }
}

// ─── Error banner ─────────────────────────────────────────────────────────────

#[component]
fn ErrorBanner(message: String) -> Element {
    let mut state: AppState = use_context();

    rsx! {
        div {
            style: "
                display: flex;
                align-items: center;
                justify-content: space-between;
                background: #1f0e0e;
                border-bottom: 1px solid #3d1515;
                padding: 10px 20px;
                font-size: 12px;
                color: #e06060;
                flex-shrink: 0;
            ",
            span { "{message}" }
            button {
                onclick: move |_| *state.error.write() = None,
                style: "
                    background: none;
                    border: none;
                    color: #e06060;
                    cursor: pointer;
                    font-size: 16px;
                    padding: 0 4px;
                    opacity: 0.7;
                ",
                "×"
            }
        }
    }
}