// ui/settings.rs
//
// Settings screen: API key management, Mods folder path override,
// cache TTL, and display preferences.
// All changes are saved to disk immediately on confirmation.

use dioxus::prelude::*;
use crate::AppState;
use crate::api::config;

// ─── Settings view ────────────────────────────────────────────────────────────

#[component]
pub fn Settings() -> Element {
    let state: AppState = use_context();
    let config = state.config.read().clone();

    rsx! {
        div {
            style: "
                flex: 1;
                overflow-y: auto;
                padding: 28px 32px;
                max-width: 640px;
            ",

            // Page heading
            div {
                style: "
                    font-size: 11px;
                    font-weight: 600;
                    letter-spacing: 0.1em;
                    text-transform: uppercase;
                    color: #4b5563;
                    margin-bottom: 24px;
                ",
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
                *status_msg.write() = Some(("API key saved to keychain.".into(), false));
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

    rsx! {
        Section {
            title: "Nexus Mods API Key",
            description: "Required to check for updates and download mods. Get your key from nexusmods.com → account → API Keys.",

            if has_key {
                div {
                    style: "display: flex; align-items: center; gap: 10px;",
                    div {
                        style: "
                            background: #1a2d1f;
                            color: #7ec8a4;
                            font-size: 12px;
                            padding: 7px 14px;
                            border-radius: 6px;
                            flex: 1;
                        ",
                        "●●●●●●●●●●●●●●●● (saved in keychain)"
                    }
                    ActionButton {
                        label: "Remove",
                        danger: true,
                        onclick: on_delete,
                    }
                }
            } else {
                div {
                    style: "display: flex; gap: 8px;",
                    input {
                        r#type: "password",
                        placeholder: "Paste your Nexus API key…",
                        value: "{key_input}",
                        oninput: move |e| *key_input.write() = e.value(),
                        style: "
                            flex: 1;
                            background: #0c0e14;
                            border: 1px solid #1e2130;
                            border-radius: 6px;
                            padding: 7px 12px;
                            font-size: 13px;
                            font-family: inherit;
                            color: #e8e6df;
                            outline: none;
                        ",
                    }
                    ActionButton {
                        label: "Save",
                        danger: false,
                        onclick: on_save,
                    }
                }
            }

            if let Some((msg, is_error)) = status {
                div {
                    style: "
                        margin-top: 8px;
                        font-size: 12px;
                        color: {if is_error { \"#e06060\" } else { \"#7ec8a4\" }};
                    ",
                    "{msg}"
                }
            }
        }
    }
}

// ─── Mods path section ────────────────────────────────────────────────────────

#[component]
fn ModsPathSection() -> Element {
    let mut state: AppState = use_context();
    let config = state.config.read().clone();

    let default_path = crate::paths::default_mods_path()
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

        if !raw.is_empty() && !crate::paths::looks_like_mods_folder(&path) {
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

    rsx! {
        Section {
            title: "Mods Folder",
            description: "Override the default Steam path if you installed Stardew Valley elsewhere.",

            // Default path hint
            div {
                style: "font-size: 11px; color: #374151; margin-bottom: 8px;",
                "Default: {default_path}"
            }

            div {
                style: "display: flex; gap: 8px;",
                input {
                    r#type: "text",
                    placeholder: "Custom path (leave blank to use default)…",
                    value: "{path_input}",
                    oninput: move |e| *path_input.write() = e.value(),
                    style: "
                        flex: 1;
                        background: #0c0e14;
                        border: 1px solid #1e2130;
                        border-radius: 6px;
                        padding: 7px 12px;
                        font-size: 12px;
                        font-family: inherit;
                        color: #e8e6df;
                        outline: none;
                    ",
                }
                ActionButton { label: "Set", danger: false, onclick: on_save }
                ActionButton { label: "Clear", danger: true, onclick: on_clear }
            }

            if let Some((msg, is_error)) = status {
                div {
                    style: "
                        margin-top: 8px;
                        font-size: 12px;
                        color: {if is_error { \"#f0a050\" } else { \"#7ec8a4\" }};
                    ",
                    "{msg}"
                }
            }
        }
    }
}

// ─── Preferences section ──────────────────────────────────────────────────────

#[component]
fn PreferencesSection() -> Element {
    let mut state: AppState = use_context();
    let config = state.config.read().clone();

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

    rsx! {
        Section {
            title: "Preferences",
            description: "Display and caching options.",

            // Show unknown source mods toggle
            ToggleRow {
                label: "Show mods with no update source",
                description: "Mods without a Nexus update key will appear in the list.",
                checked: config.show_unknown_source_mods,
                onchange: on_toggle_unknown,
            }

            // Cache TTL
            div {
                style: "margin-top: 16px;",
                div {
                    style: "font-size: 12px; color: #9ca3af; margin-bottom: 6px;",
                    "Cache duration (seconds)"
                }
                div {
                    style: "display: flex; align-items: center; gap: 10px;",
                    select {
                        value: "{config.cache_ttl_seconds}",
                        onchange: on_ttl_change,
                        style: "
                            background: #0c0e14;
                            border: 1px solid #1e2130;
                            border-radius: 6px;
                            padding: 6px 10px;
                            font-size: 12px;
                            font-family: inherit;
                            color: #e8e6df;
                            outline: none;
                        ",
                        option { value: "1800",  "30 minutes" }
                        option { value: "3600",  "1 hour (default)" }
                        option { value: "7200",  "2 hours" }
                        option { value: "86400", "24 hours" }
                    }
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
        match crate::updater::prune_backups(30, 3) {
            Ok(n) => *status_msg.write() = Some((
                format!("Deleted {n} old backup{}", if n == 1 { "" } else { "s" }),
                false,
            )),
            Err(e) => *status_msg.write() = Some((format!("Prune failed: {e}"), true)),
        }
    };

    let status = status_msg.read().clone();

    rsx! {
        Section {
            title: "Backups",
            description: "Before each update, the old mod version is zipped and saved. Prune removes backups older than 30 days, keeping at least 3 per mod.",

            ActionButton { label: "Prune old backups", danger: false, onclick: on_prune }

            if let Some((msg, is_error)) = status {
                div {
                    style: "
                        margin-top: 8px;
                        font-size: 12px;
                        color: {if is_error { \"#e06060\" } else { \"#7ec8a4\" }};
                    ",
                    "{msg}"
                }
            }
        }
    }
}

// ─── Shared layout components ─────────────────────────────────────────────────

/// A titled settings section with a description.
#[component]
fn Section(title: &'static str, description: &'static str, children: Element) -> Element {
    rsx! {
        div {
            style: "margin-bottom: 28px;",

            div {
                style: "font-size: 13px; font-weight: 600; color: #d1cfc8; margin-bottom: 4px;",
                "{title}"
            }
            div {
                style: "font-size: 12px; color: #4b5563; margin-bottom: 14px; line-height: 1.5;",
                "{description}"
            }

            { children }
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
    let bg     = if checked { "#7ec8a4" } else { "#1e2130" };
    let offset = if checked { "translateX(16px)" } else { "translateX(2px)" };

    rsx! {
        div {
            style: "display: flex; align-items: flex-start; gap: 12px; cursor: pointer;",
            onclick: move |e| onchange.call(e),

            // Toggle track
            div {
                style: "
                    width: 36px;
                    height: 20px;
                    border-radius: 10px;
                    background: {bg};
                    position: relative;
                    flex-shrink: 0;
                    margin-top: 1px;
                    transition: background 0.15s;
                ",
                div {
                    style: "
                        position: absolute;
                        top: 2px;
                        width: 16px;
                        height: 16px;
                        border-radius: 50%;
                        background: #0c0e14;
                        transform: {offset};
                        transition: transform 0.15s;
                    "
                }
            }

            div {
                div {
                    style: "font-size: 13px; color: #9ca3af;",
                    "{label}"
                }
                div {
                    style: "font-size: 11px; color: #374151; margin-top: 2px;",
                    "{description}"
                }
            }
        }
    }
}

/// A small action button. `danger: true` renders in red.
#[component]
fn ActionButton(label: &'static str, danger: bool, onclick: EventHandler<MouseEvent>) -> Element {
    let bg    = if danger { "#2d1519" } else { "#1a2035" };
    let color = if danger { "#e06060" } else { "#7ec8a4" };

    rsx! {
        button {
            onclick: move |e| onclick.call(e),
            style: "
                background: {bg};
                color: {color};
                border: none;
                padding: 7px 14px;
                border-radius: 6px;
                font-size: 12px;
                font-family: inherit;
                font-weight: 600;
                cursor: pointer;
                letter-spacing: 0.02em;
                white-space: nowrap;
            ",
            "{label}"
        }
    }
}

/// A thin horizontal rule between sections.
#[component]
fn Divider() -> Element {
    rsx! {
        div {
            style: "
                border-top: 1px solid #1e2130;
                margin: 4px 0 28px;
            "
        }
    }
}