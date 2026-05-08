// ui/install_drop.rs
//
// Two install entry points:
//   1. A drag-and-drop zone
//   2. A "Browse…" button that opens a native file picker
//
// Both feed into installer::install_from_zip().

use dioxus::prelude::*;
use dioxus::html::HasFileData;
use rfd::FileDialog;
use crate::api::app_state::AppState;
use crate::api::installer;
use crate::api::mod_manager::ModStatus;

#[component]
pub fn InstallZone() -> Element {
    let state: AppState = use_context();
    let state_for_browse = state.clone();
    let state_for_drop = state.clone();
    let mut install_msg = use_signal(|| Option::<(String, bool)>::None);
    let mut drag_over   = use_signal(|| false);

    // ── File picker ──────────────────────────────────────────────────────────
    let on_browse = move |_| {
        let mut state = state_for_browse.clone();
        spawn(async move {
            // rfd opens a native macOS Open panel — blocks until user picks
            let picked = FileDialog::new()
                .add_filter("Mod zip", &["zip"])
                .set_title("Select a mod zip file")
                .pick_file();

            if let Some(path) = picked {
                run_install(path, &mut state, &mut install_msg).await;
            }
        });
    };

    // ── Drag and drop ────────────────────────────────────────────────────────
    // Dioxus desktop surfaces file drag events via ondragover / ondrop.
    // The file path comes through as the drag data value.
    let on_drag_over = move |evt: DragEvent| {
        evt.prevent_default();
        *drag_over.write() = true;
    };

    let on_drag_leave = move |_| {
        *drag_over.write() = false;
    };

    let on_drop = move |evt: DragEvent| {
        evt.prevent_default();
        *drag_over.write() = false;

        let dropped_zip = evt
            .files()
            .into_iter()
            .map(|file| std::path::PathBuf::from(file.name()))
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("zip"));

        let mut state = state_for_drop.clone();
        spawn(async move {
            if let Some(zip_path) = dropped_zip {
                run_install(zip_path, &mut state, &mut install_msg).await;
            }
        });
    };

    let border_color = if *drag_over.read() { "#7ec8a4" } else { "#1e2130" };
    let bg           = if *drag_over.read() { "#0d1f14" } else { "#0c0e14" };
    let msg_color = install_msg
        .read()
        .as_ref()
        .map(|(_, is_error)| if *is_error { "#e06060" } else { "#7ec8a4" })
        .unwrap_or("#7ec8a4");

    rsx! {
        div { class: "max-w-2xl mx-auto w-full px-5 py-6",

            div { class: "mb-4",
                div { class: "text-[11px] font-semibold tracking-widest uppercase text-[#9ca3af] mb-1",
                    "Install Mod"
                }
                div { class: "text-xs text-[#b0b8c7]",
                    "Drop a zip or browse for one. Existing installs are backed up before replacement."
                }
            }

            div { class: "flex flex-col gap-3",

                // Drop zone
                div {
                    ondragover: on_drag_over,
                    ondragleave: on_drag_leave,
                    ondrop: on_drop,
                    class: "flex flex-col items-center justify-center gap-3 \
                        rounded-lg border-2 border-dashed p-10 \
                        transition-colors duration-150 cursor-pointer",
                    style: "border-color: {border_color}; background: {bg};",

                    div { class: "text-3xl opacity-40", "⬇" }
                    div { class: "text-sm text-[#b0b8c7]", "Drop a mod zip here" }

                    // Browse button
                    button {
                        onclick: on_browse,
                        class: "mt-1 px-4 py-1.5 rounded-md text-xs font-semibold \
                            bg-[#1a2035] border border-[#2d3552] text-[#7ec8a4] \
                            hover:border-[#7ec8a4] hover:text-[#7ec8a4] \
                            transition-colors duration-150",
                        "Browse..."
                    }
                }

                // Result message
                if let Some((msg, _is_error)) = install_msg.read().clone() {
                    div { class: "text-xs px-1", style: "color: {msg_color};", "{msg}" }
                }
            }
        }
    }
}

/// Shared install logic called by both picker and drag-drop paths.
async fn run_install(
    zip_path: std::path::PathBuf,
    state: &mut AppState,
    msg: &mut Signal<Option<(String, bool)>>,
) {
    let config = state.config.read().clone();

    let Some(mods_dir) = config.resolved_mods_path() else {
        *msg.write() = Some(("No Mods folder configured. Set one in Settings.".into(), true));
        return;
    };

    match installer::install_from_zip(&zip_path, &mods_dir) {
        Ok(result) => {
            let verb = if result.was_update { "Updated" } else { "Installed" };
            *msg.write() = Some((
                format!("{verb}: {} v{}", result.manifest.name, result.manifest.version),
                false,
            ));

            // Append to or update the mod list in state so UI reflects immediately
            let mut mods = state.mods.write();
            if result.was_update {
                if let Some(m) = mods.iter_mut().find(|m| m.manifest.unique_id == result.manifest.unique_id) {
                    m.manifest = result.manifest;
                    m.status = ModStatus::UpToDate;
                }
            } else {
                // New mod — push it and let the user rescan for full status
                // (we don't have a path reference here without a rescan)
                // Simplest: just signal that a rescan would be helpful
                *msg.write() = Some((
                    format!("Installed {}. Press Scan Mods to refresh the list.", result.manifest.name),
                    false,
                ));
            }
        }
        Err(e) => {
            *msg.write() = Some((format!("Install failed: {e}"), true));
        }
    }
}