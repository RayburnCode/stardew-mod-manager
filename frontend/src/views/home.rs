

use dioxus::prelude::*;


use crate::mod_list::ModList;
use crate::settings::Settings;

// ─── App state ────────────────────────────────────────────────────────────────



// ─── Entry point ──────────────────────────────────────────────────────────────


#[component]
pub fn Home() -> Element {
    // Load config once on startup
    let config = config::AppConfig::load().unwrap_or_default();

    let state = AppState {
        screen:     use_signal(|| Screen::ModList),
        config:     use_signal(|| config),
        mods:       use_signal(Vec::new),
        loading:    use_signal(|| false),
        error:      use_signal(|| None),
        is_premium: use_signal(|| None),
    };

    // Make state available to all child components
    use_context_provider(|| state.clone());

    let screen = state.screen.read().clone();

    rsx! {
        div {
            class: "app-shell",
            style: "
                font-family: 'Berkeley Mono', 'Fira Code', 'JetBrains Mono', monospace;
                background: #0f1117;
                color: #e8e6df;
                height: 100vh;
                display: flex;
                flex-direction: column;
                overflow: hidden;
            ",

            // ── Top bar ──────────────────────────────────────────────────────
            TopBar {}

            // ── Main content area ─────────────────────────────────────────────
            div { style: "flex: 1; overflow: hidden; display: flex; flex-direction: column;",
                match screen {
                    Screen::ModList => rsx! {
                        ModList {}
                    },
                    Screen::Settings => rsx! {
                        Settings {}
                    },
                }
            }
        }
    }
}

// ─── Top bar ──────────────────────────────────────────────────────────────────

#[component]
fn TopBar() -> Element {
    let mut state: AppState = use_context();
    let screen = state.screen.read().clone();

    rsx! {
        div { style: "
                display: flex;
                align-items: center;
                justify-content: space-between;
                padding: 0 20px;
                height: 52px;
                border-bottom: 1px solid #1e2130;
                background: #0c0e14;
                flex-shrink: 0;
            ",

            // App name
            span { style: "
                    font-size: 13px;
                    font-weight: 600;
                    letter-spacing: 0.08em;
                    color: #7ec8a4;
                    text-transform: uppercase;
                ",
                "Stardew Mod Manager"
            }

            // Nav tabs
            div { style: "display: flex; gap: 2px;",

                NavTab {
                    label: "Mods",
                    active: screen == Screen::ModList,
                    onclick: move |_| *state.screen.write() = Screen::ModList,
                }
                NavTab {
                    label: "Settings",
                    active: screen == Screen::Settings,
                    onclick: move |_| *state.screen.write() = Screen::Settings,
                }
            }
        }
    }
}

#[component]
fn NavTab(label: &'static str, active: bool, onclick: EventHandler<MouseEvent>) -> Element {
    let bg    = if active { "#1a2035" } else { "transparent" };
    let color = if active { "#7ec8a4" } else { "#6b7280" };

    rsx! {
        button {
            onclick: move |e| onclick.call(e),
            style: "
                background: {bg};
                color: {color};
                border: none;
                padding: 6px 16px;
                border-radius: 6px;
                font-size: 13px;
                font-family: inherit;
                cursor: pointer;
                transition: color 0.15s;
                letter-spacing: 0.02em;
            ",
            "{label}"
        }
    }
}