// ui/mod_list.rs
//
// The main view: a table of installed mods with their status,
// a toolbar for scanning/refreshing, and per-row update actions.

use dioxus::prelude::*;
use crate::api::app_state::AppState;
use crate::api::mod_manager::{InstalledMod, ModStatus};
//use crate::api::config;
use crate::api::nexus::NexusClient;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── ModList view ─────────────────────────────────────────────────────────────

#[component]
pub fn ModList() -> Element {
    let state: AppState = use_context();
    let mods    = state.mods.read();
    let loading = *state.loading.read();
    let error   = state.error.read().clone();

    rsx! {
        div { class: "flex flex-col h-full overflow-hidden",

            // ── Toolbar ───────────────────────────────────────────────────────
            Toolbar {}

            // ── Error banner ──────────────────────────────────────────────────
            if let Some(err) = error {
                ErrorBanner { message: err }
            }

            // ── Content area ──────────────────────────────────────────────────
            div { class: "flex-1 overflow-y-auto px-5 pb-5",

                if loading {
                    LoadingState {}
                } else if mods.is_empty() {
                    EmptyState {}
                } else {
                    // Summary row
                    {
                        let total = mods.len();
                        let updates = mods
                            .iter()
                            .filter(|m| matches!(m.status, ModStatus::UpdateAvailable { .. }))
                            .count();
                        rsx! {
                            SummaryBar { total, updates }
                        }
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
    let state: AppState = use_context();
    let state_for_scan = state.clone();
    let state_for_check = state.clone();
    let loading = *state.loading.read();

    // Scan mods folder only. Network checks are handled separately.
    let on_scan = move |_| {
        let mut state = state_for_scan.clone();
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
            *state.loading.write() = false;
        });
    };

    // Check updates via both SMAPI (all mods, no key needed) and Nexus (if API
    // key is configured). Results are merged: whichever source reports the
    // newer version wins.
    let on_check_updates = move |_| {
        let mut state = state_for_check.clone();
        spawn(async move {
            *state.error.write() = None;
            *state.loading.write() = true;

            let config = state.config.read().clone();
            let base_mods = if state.mods.read().is_empty() {
                let Some(mods_path) = config.resolved_mods_path() else {
                    *state.error.write() = Some(
                        "No Stardew Valley Mods folder found. Set a path in Settings.".into()
                    );
                    *state.loading.write() = false;
                    return;
                };
                match crate::api::mod_manager::discover_mods(&mods_path) {
                    Ok(m) => m,
                    Err(e) => {
                        *state.error.write() = Some(format!("Scan failed: {e}"));
                        *state.loading.write() = false;
                        return;
                    }
                }
            } else {
                state.mods.read().clone()
            };

            if base_mods.is_empty() {
                *state.mods.write() = base_mods;
                *state.loading.write() = false;
                return;
            }

            // Fetch from Nexus only
            let nexus_results = if config.nexus_api_key_saved {
                if let Ok(Some(api_key)) = crate::api::config::load_api_key() {
                    match NexusClient::new(&api_key, config.cache_ttl_seconds) {
                        Ok(mut client) => {
                            let nexus_ids: Vec<u32> = base_mods
                                .iter()
                                .filter_map(|m| m.manifest.nexus_id())
                                .collect();
                            client.fetch_many_with_versions(&nexus_ids).await
                        }
                        Err(_) => std::collections::HashMap::new(),
                    }
                } else {
                    std::collections::HashMap::new()
                }
            } else {
                std::collections::HashMap::new()
            };

            // Update mod statuses based on Nexus data
            let updated_mods: Vec<InstalledMod> = base_mods
                .into_iter()
                .map(|mut m| {
                    m.status = if let Some(nexus_id) = m.manifest.nexus_id() {
                        // Mod has Nexus key — check Nexus for updates
                        nexus_results.get(&nexus_id)
                            .and_then(|r| r.as_ref().ok())
                            .and_then(|(_, version_info)| {
                                if crate::api::mod_manager::is_update_available(&m.manifest.version, &version_info.version) {
                                    Some(ModStatus::UpdateAvailable {
                                        latest: version_info.version.clone(),
                                        updated_timestamp: Some(version_info.updated_timestamp),
                                    })
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(ModStatus::UpToDate)
                    } else {
                        // No Nexus key — can't check for updates
                        ModStatus::Unknown
                    };
                    m
                })
                .collect();

            *state.mods.write() = updated_mods;
            *state.loading.write() = false;
        });
    };

    rsx! {
        div { class: "flex items-center justify-between gap-4 px-5 py-3.5 border-b border-[#1e2130] shrink-0",
            div { class: "flex flex-col gap-1",
                span { class: "text-xs uppercase tracking-[0.08em] text-[#9ca3af]",
                    "Step 1: Scan   Step 2: Check updates"
                }
                span { class: "text-xs text-[#aab0bb]",
                    "Scan reads your Mods folder only. Check updates compares installed versions with Nexus."
                }
            }

            div { class: "flex items-center gap-2.5",
                button {
                    onclick: on_scan,
                    disabled: loading,
                    class: if loading { "bg-[#7ec8a4] text-[#0c0e14] border-none py-[7px] px-4 rounded-md text-[13px] font-[inherit] font-semibold tracking-[0.02em] opacity-60 cursor-not-allowed" } else { "bg-[#7ec8a4] text-[#0c0e14] border-none py-[7px] px-4 rounded-md text-[13px] font-[inherit] font-semibold cursor-pointer tracking-[0.02em]" },
                    if loading {
                        "Scanning..."
                    } else {
                        "Scan Mods Folder"
                    }
                }

                button {
                    onclick: on_check_updates,
                    disabled: loading,
                    class: if loading { "bg-[#1a2035] text-[#7ec8a4] border border-[#2d3552] py-[7px] px-3.5 rounded-md text-[13px] font-[inherit] font-semibold tracking-[0.02em] opacity-60 cursor-not-allowed" } else { "bg-[#1a2035] text-[#7ec8a4] border border-[#2d3552] py-[7px] px-3.5 rounded-md text-[13px] font-[inherit] font-semibold cursor-pointer tracking-[0.02em]" },
                    "Check for Updates"
                }
            }
        }
    }
}

// ─── Summary bar ──────────────────────────────────────────────────────────────

#[component]
fn SummaryBar(total: usize, updates: usize) -> Element {
    let update_label = format!(
        "{updates} update{} available",
        if updates == 1 { "" } else { "s" }
    );

    rsx! {
        div { class: "flex items-center gap-4 py-3.5 pb-2.5 text-xs text-[#aab0bb]",
            span { "{total} mods installed" }
            if updates > 0 {
                span { class: "rounded-full bg-[#1a2d1f] px-2.5 py-0.5 font-semibold text-[#7ec8a4]",
                    "{update_label}"
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
            class: "grid gap-3 px-3 py-1.5 text-[11px] font-semibold tracking-[0.08em] uppercase text-[#9ca3af] border-b border-[#1e2130] mb-1",
            style: "grid-template-columns: 1fr 120px 120px 140px;",
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
    let state: AppState = use_context();
    let name    = mod_data.manifest.name.clone();
    let author  = mod_data.manifest.author.clone();
    let version = mod_data.manifest.version.clone();

    let (latest_text, latest_age_text, status_el) = match &mod_data.status {
        ModStatus::UpToDate => (
            version.clone(),
            None,
            rsx! {
                StatusPill {
                    color: "#1a2d1f",
                    text_color: "#7ec8a4",
                    label: "Up to date",
                }
            },
        ),
        ModStatus::UpdateAvailable { latest, updated_timestamp } => {
            let latest = latest.clone();
            let mod_path = mod_data.path.clone();
            let dl_latest = latest.clone();
            let updated_ago = updated_timestamp.and_then(format_relative_age);

            let on_update = move |_| {
                let mut state = state.clone();
                let mod_path = mod_path.clone();
                let expected = dl_latest.clone();

                spawn(async move {
                    *state.error.write() = None;

                    // Get the API key and build client
                    let config = state.config.read().clone();
                    let api_key = match crate::api::config::load_api_key() {
                        Ok(Some(k)) => k,
                        _ => {
                            *state.error.write() = Some("No API key — add one in Settings.".into());
                            return;
                        }
                    };

                    let client = match NexusClient::new(&api_key, config.cache_ttl_seconds) {
                        Ok(c) => c,
                        Err(e) => {
                            *state.error.write() = Some(format!("Client error: {e}"));
                            return;
                        }
                    };

                    // Get Nexus ID from the mod's manifest
                    let nexus_id = match crate::api::mod_manager::discover_mods(&mod_path.parent().unwrap_or(&mod_path))
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
                    match crate::api::updater::install_update(&mod_path, &url, &expected, None).await {
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
                updated_ago,
                rsx! {
                    div { class: "flex items-center gap-2",
                        StatusPill {
                            color: "#2d1f0a",
                            text_color: "#f0a050",
                            label: "Update",
                        }
                        button {
                            onclick: on_update,
                            class: "bg-[#f0a050] text-[#0c0e14] border-none py-[3px] px-2.5 rounded-[5px] text-[11px] font-[inherit] font-bold cursor-pointer tracking-[0.04em]",
                            "↑ Install"
                        }
                    }
                },
            )
        }
        ModStatus::FetchFailed { .. } => (
            "—".into(),
            None,
            rsx! {
                StatusPill {
                    color: "#2d1519",
                    text_color: "#e06060",
                    label: "Fetch error",
                }
            },
        ),
        ModStatus::Unknown => (
            "—".into(),
            None,
            rsx! {
                StatusPill {
                    color: "#1a1c24",
                    text_color: "#aab0bb",
                    label: "No source",
                }
            },
        ),
    };

    rsx! {
        div {
            class: "grid gap-3 items-center px-3 py-2.5 rounded-[7px] mb-0.5 bg-[#0f1117] border border-[#1e2130] transition-colors duration-150",
            style: "grid-template-columns: 1fr 120px 120px 140px;",

            // Name + author
            div {
                div { class: "text-[13px] text-[#e8e6df] font-medium", "{name}" }
                div { class: "text-[11px] text-[#aab0bb] mt-px", "{author}" }
            }

            // Installed version
            span { class: "text-xs text-[#b0b8c7] tabular-nums", "{version}" }

            // Latest version + relative update age if known
            div { class: "flex flex-col",
                span { class: "text-xs text-[#b0b8c7] tabular-nums", "{latest_text}" }
                if let Some(age) = latest_age_text {
                    span { class: "text-[10px] text-[#8d95a3]", "Updated {age}" }
                }
            }

            // Status / action
            {status_el}
        }
    }
}

fn format_relative_age(updated_timestamp: u64) -> Option<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();

    if updated_timestamp > now {
        return None;
    }

    let delta = now.saturating_sub(updated_timestamp);
    if delta < 60 {
        return Some("just now".to_string());
    }

    if delta < 3_600 {
        let mins = delta / 60;
        return Some(format!("{mins} min{} ago", if mins == 1 { "" } else { "s" }));
    }

    if delta < 86_400 {
        let hours = delta / 3_600;
        return Some(format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" }));
    }

    if delta < 604_800 {
        let days = delta / 86_400;
        return Some(format!("{days} day{} ago", if days == 1 { "" } else { "s" }));
    }

    if delta < 2_629_746 {
        let weeks = delta / 604_800;
        return Some(format!("{weeks} week{} ago", if weeks == 1 { "" } else { "s" }));
    }

    if delta < 31_556_952 {
        let months = delta / 2_629_746;
        return Some(format!("{months} month{} ago", if months == 1 { "" } else { "s" }));
    }

    let years = delta / 31_556_952;
    Some(format!("{years} year{} ago", if years == 1 { "" } else { "s" }))
}

// ─── Status pill ──────────────────────────────────────────────────────────────

#[component]
fn StatusPill(color: &'static str, text_color: &'static str, label: &'static str) -> Element {
    rsx! {
        span {
            class: "inline-block text-[11px] font-semibold py-[3px] px-2.5 rounded-full tracking-[0.04em]",
            style: "background: {color}; color: {text_color};",
            "{label}"
        }
    }
}

// ─── Loading state ────────────────────────────────────────────────────────────

#[component]
fn LoadingState() -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center h-[300px] gap-3 text-[#b0b8c7] text-[13px]",
            span { class: "text-2xl", "⟳" }
            span { "Scanning mods…" }
        }
    }
}

// ─── Empty state ──────────────────────────────────────────────────────────────

#[component]
fn EmptyState() -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center h-[300px] gap-2.5 text-[#aab0bb] text-center",
            div { class: "text-[32px] opacity-40", "⬡" }
            div { class: "text-sm text-[#b0b8c7]", "No mods scanned yet" }
            div { class: "text-xs text-[#9ca3af]", "Press Scan Mods to find your installed mods" }
        }
    }
}

// ─── Error banner ─────────────────────────────────────────────────────────────

#[component]
fn ErrorBanner(message: String) -> Element {
    let mut state: AppState = use_context();

    rsx! {
        div { class: "flex items-center justify-between bg-[#1f0e0e] border-b border-[#3d1515] px-5 py-2.5 text-xs text-[#e06060] shrink-0",
            span { "{message}" }
            button {
                onclick: move |_| *state.error.write() = None,
                class: "bg-transparent border-none text-[#e06060] cursor-pointer text-base px-1 opacity-70",
                "×"
            }
        }
    }
}