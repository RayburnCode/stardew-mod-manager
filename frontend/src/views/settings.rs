// ui/settings.rs
//
// Settings screen: API key management, Mods folder path override,
// cache TTL, and display preferences.
// All changes are saved to disk immediately on confirmation.

use dioxus::prelude::*;
use crate::api::app_state::AppState;
use crate::api::config;

// ─── Settings view ────────────────────────────────────────────────────────────

#[component]
pub fn Settings() -> Element {
    let state: AppState = use_context();

    rsx! {
        div { class: "flex-1 overflow-y-auto py-7 px-8 max-w-2xl",

            // Page heading
            div { class: "text-[11px] font-semibold tracking-widest uppercase text-[#9ca3af] mb-6",
                "Settings"
            }

            // Nexus API key section
            ApiKeySection {}

            Divider {}

            // Mods path section
            ModsPathSection {}

            Divider {}

            // Preferences section
            PreferencesSection {}

            Divider {}

            // Backup management
            BackupSection {}
        }
    }
}

// ─── API Key section ──────────────────────────────────────────────────────────
 
#[component]
fn ApiKeySection() -> Element {
    let mut state: AppState = use_context();
    let config = state.config.read().clone();

    let mut key_input = use_signal(String::new);
    let mut status_msg = use_signal(|| Option::<(String, bool)>::None); // (msg, is_error)

    let has_key = config.nexus_api_key_saved;

    let on_save = move |_| {
        let key = key_input.read().trim().to_string();
        if key.is_empty() {
            *status_msg.write() = Some(("Key cannot be empty.".into(), true));
            return;
        }

        let mut config = state.config.read().clone();
        match config::save_api_key(&mut config, &key) {
            Ok(_) => {
                *state.config.write() = config;
                *key_input.write() = String::new();
                    *status_msg.write() = Some(("API key saved locally.".into(), false));
            }
            Err(e) => {
                *status_msg.write() = Some((format!("Save failed: {e}"), true));
            }
        }
    };

    let on_delete = move |_| {
        let mut config = state.config.read().clone();
        match config::delete_api_key(&mut config) {
            Ok(_) => {
                *state.config.write() = config;
                *status_msg.write() = Some(("API key removed.".into(), false));
            }
            Err(e) => {
                *status_msg.write() = Some((format!("Remove failed: {e}"), true));
            }
        }
    };

    let status = status_msg.read().clone();
    let status_color = status.as_ref().map(|(_, err)| if *err { "#e06060" } else { "#7ec8a4" }).unwrap_or("transparent");

    rsx! {
        Section {
            title: "Nexus Mods API Key",
            description: "Required to check for updates and download mods. Get your key from nexusmods.com → account → API Keys.",

            if has_key {
                div { class: "flex items-center gap-2.5",
                    div { class: "bg-[#1a2d1f] text-[#7ec8a4] text-xs py-1.5 px-3.5 rounded-md flex-1",
                        "●●●●●●●●●●●●●●●● (saved locally)"
                    }
                    ActionButton { label: "Remove", danger: true, onclick: on_delete }
                }
            } else {
                div { class: "flex gap-2",
                    input {
                        r#type: "password",
                        placeholder: "Paste your Nexus API key…",
                        value: "{key_input}",
                        oninput: move |e| *key_input.write() = e.value(),
                        class: "flex-1 bg-[#0c0e14] border border-[#1e2130] rounded-md py-1.5 px-3 text-[13px] font-[inherit] text-[#e8e6df] outline-none",
                    }
                    ActionButton { label: "Save", danger: false, onclick: on_save }
                }
            }

            if let Some((msg, _)) = status {
                div { class: "mt-2 text-xs", style: "color: {status_color};", "{msg}" }
            }
        }
    }
}

// ─── Mods path section ────────────────────────────────────────────────────────

#[component]
fn ModsPathSection() -> Element {
    let mut state: AppState = use_context();
    let config = state.config.read().clone();

    let default_path = crate::api::paths::default_mods_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "Not detected".into());

    let override_path = config.mods_path_override
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut path_input = use_signal(|| override_path.clone());
    let mut status_msg = use_signal(|| Option::<(String, bool)>::None);

    let on_save = move |_| {
        let raw = path_input.read().trim().to_string();
        let path = std::path::PathBuf::from(&raw);

        if !raw.is_empty() && !crate::api::paths::looks_like_mods_folder(&path) {
            *status_msg.write() = Some((
                "Warning: path doesn't look like a Stardew Mods folder. Saved anyway.".into(),
                true,
            ));
        } else {
            *status_msg.write() = Some(("Path saved.".into(), false));
        }

        let mut config = state.config.read().clone();
        config.mods_path_override = if raw.is_empty() { None } else { Some(path) };

        if let Err(e) = config.save() {
            *status_msg.write() = Some((format!("Save failed: {e}"), true));
            return;
        }
        *state.config.write() = config;
    };

    let on_clear = move |_| {
        let mut config = state.config.read().clone();
        config.mods_path_override = None;
        let _ = config.save();
        *state.config.write() = config.clone();
        *path_input.write() = String::new();
        *status_msg.write() = Some(("Using OS default path.".into(), false));
    };

    let status = status_msg.read().clone();
    let status_color = status.as_ref().map(|(_, err)| if *err { "#f0a050" } else { "#7ec8a4" }).unwrap_or("transparent");

    rsx! {
        Section {
            title: "Mods Folder",
            description: "Override the default Steam path if you installed Stardew Valley elsewhere.",

            // Default path hint
            div { class: "text-[11px] text-[#9ca3af] mb-2", "Default: {default_path}" }

            div { class: "flex gap-2",
                input {
                    r#type: "text",
                    placeholder: "Custom path (leave blank to use default)…",
                    value: "{path_input}",
                    oninput: move |e| *path_input.write() = e.value(),
                    class: "flex-1 bg-[#0c0e14] border border-[#1e2130] rounded-md py-1.5 px-3 text-xs font-[inherit] text-[#e8e6df] outline-none",
                }
                ActionButton { label: "Set", danger: false, onclick: on_save }
                ActionButton { label: "Clear", danger: true, onclick: on_clear }
            }

            if let Some((msg, _)) = status {
                div { class: "mt-2 text-xs", style: "color: {status_color};", "{msg}" }
            }
        }
    }
}

// ─── Preferences section ──────────────────────────────────────────────────────

#[component]
fn PreferencesSection() -> Element {
    let mut state: AppState = use_context();
    let config = state.config.read().clone();

    let mut cache_status = use_signal(|| Option::<(String, bool)>::None);

    let on_toggle_unknown = move |_| {
        let mut config = state.config.read().clone();
        config.show_unknown_source_mods = !config.show_unknown_source_mods;
        let _ = config.save();
        *state.config.write() = config;
    };

    let on_ttl_change = move |e: Event<FormData>| {
        if let Ok(secs) = e.value().parse::<u64>() {
            let mut config = state.config.read().clone();
            config.cache_ttl_seconds = secs;
            let _ = config.save();
            *state.config.write() = config;
        }
    };

    let on_clear_cache = move |_| {
        match crate::api::paths::nexus_cache_file() {
            Ok(path) => {
                if path.exists() {
                    match std::fs::remove_file(&path) {
                        Ok(_) => *cache_status.write() = Some(("Nexus cache cleared.".into(), false)),
                        Err(e) => *cache_status.write() = Some((format!("Failed to clear cache: {e}"), true)),
                    }
                } else {
                    *cache_status.write() = Some(("Cache is already empty.".into(), false));
                }
            }
            Err(e) => *cache_status.write() = Some((format!("Could not locate cache: {e}"), true)),
        }
    };

    let cache_st = cache_status.read().clone();
    let cache_color = cache_st.as_ref().map(|(_, err)| if *err { "#e06060" } else { "#7ec8a4" }).unwrap_or("transparent");

    rsx! {
        Section { title: "Preferences", description: "Display and caching options.",

            // Show unknown source mods toggle
            ToggleRow {
                label: "Show mods with no update source",
                description: "Mods without a Nexus update key will appear in the list.",
                checked: config.show_unknown_source_mods,
                onchange: on_toggle_unknown,
            }

            // Cache TTL
            div { class: "mt-4",
                div { class: "text-xs text-[#b0b8c7] mb-1.5", "Cache duration" }
                div { class: "flex items-center gap-2.5",
                    select {
                        value: "{config.cache_ttl_seconds}",
                        onchange: on_ttl_change,
                        class: "bg-[#0c0e14] border border-[#1e2130] rounded-md py-1.5 px-2.5 text-xs font-[inherit] text-[#e8e6df] outline-none",
                        option { value: "0", "Never (always refresh)" }
                        option { value: "1800", "30 minutes" }
                        option { value: "3600", "1 hour (default)" }
                        option { value: "7200", "2 hours" }
                        option { value: "86400", "24 hours" }
                    }
                }
            }

            // Clear cache
            div { class: "mt-4",
                div { class: "text-xs text-[#b0b8c7] mb-1.5", "Nexus version cache" }
                div { class: "text-[11px] text-[#9ca3af] mb-2",
                    "If updates aren't showing up, clearing the cache forces a fresh fetch from Nexus on the next check."
                }
                ActionButton {
                    label: "Clear cache",
                    danger: false,
                    onclick: on_clear_cache,
                }

                if let Some((msg, _)) = cache_st {
                    div { class: "mt-2 text-xs", style: "color: {cache_color};", "{msg}" }
                }
            }
        }
    }
}

// ─── Backup section ───────────────────────────────────────────────────────────

#[component]
fn BackupSection() -> Element {
    let mut status_msg = use_signal(|| Option::<(String, bool)>::None);

    let on_prune = move |_| {
        match crate::api::updater::prune_backups(30, 3) {
            Ok(n) => *status_msg.write() = Some((
                format!("Deleted {n} old backup{}", if n == 1 { "" } else { "s" }),
                false,
            )),
            Err(e) => *status_msg.write() = Some((format!("Prune failed: {e}"), true)),
        }
    };

    let status = status_msg.read().clone();
    let status_color = status.as_ref().map(|(_, err)| if *err { "#e06060" } else { "#7ec8a4" }).unwrap_or("transparent");

    rsx! {
        Section {
            title: "Backups",
            description: "Before each update, the old mod version is zipped and saved. Prune removes backups older than 30 days, keeping at least 3 per mod.",

            ActionButton {
                label: "Prune old backups",
                danger: false,
                onclick: on_prune,
            }

            if let Some((msg, _)) = status {
                div { class: "mt-2 text-xs", style: "color: {status_color};", "{msg}" }
            }
        }
    }
}

// ─── Shared layout components ─────────────────────────────────────────────────

/// A titled settings section with a description.
#[component]
fn Section(title: &'static str, description: &'static str, children: Element) -> Element {
    rsx! {
        div { class: "mb-7",

            div { class: "text-[13px] font-semibold text-[#d1cfc8] mb-1", "{title}" }
            div { class: "text-xs text-[#aab0bb] mb-3.5 leading-relaxed", "{description}" }

            {children}
        }
    }
}

/// A labeled toggle row.
#[component]
fn ToggleRow(
    label: &'static str,
    description: &'static str,
    checked: bool,
    onchange: EventHandler<MouseEvent>,
) -> Element {
    let track_bg  = if checked { "bg-[#7ec8a4]" } else { "bg-[#1e2130]" };
    let knob_offset = if checked { "translateX(16px)" } else { "translateX(2px)" };

    rsx! {
        div {
            class: "flex items-start gap-3 cursor-pointer",
            onclick: move |e| onchange.call(e),

            // Toggle track
            div { class: "w-9 h-5 rounded-[10px] relative flex-shrink-0 mt-px transition-colors {track_bg}",
                div {
                    class: "absolute top-0.5 w-4 h-4 rounded-full bg-[#0c0e14] transition-transform",
                    style: "transform: {knob_offset};",
                }
            }

            div {
                div { class: "text-[13px] text-[#b0b8c7]", "{label}" }
                div { class: "text-[11px] text-[#9ca3af] mt-0.5", "{description}" }
            }
        }
    }
}

/// A small action button. `danger: true` renders in red.
#[component]
fn ActionButton(label: &'static str, danger: bool, onclick: EventHandler<MouseEvent>) -> Element {
    let btn_class = if danger {
        "bg-[#2d1519] text-[#e06060]"
    } else {
        "bg-[#1a2035] text-[#7ec8a4]"
    };

    rsx! {
        button {
            onclick: move |e| onclick.call(e),
            class: "border-none py-1.5 px-3.5 rounded-md text-xs font-[inherit] font-semibold cursor-pointer tracking-wide whitespace-nowrap {btn_class}",
            "{label}"
        }
    }
}

/// A thin horizontal rule between sections.
#[component]
fn Divider() -> Element {
    rsx! {
        div { class: "border-t border-[#1e2130] mt-1 mb-7" }
    }
}