use dioxus::prelude::*;

/// Which screen is currently visible.
#[derive(Clone, PartialEq)]
pub enum Screen {
    ModList,
    Installer,
    Settings,
}

/// Global app state shared across components via Dioxus context.
#[derive(Clone, PartialEq)]
pub struct AppState { 
    pub screen: Signal<Screen>,
    pub config: Signal<crate::api::config::AppConfig>,
    /// Installed mods with their update status. Empty until first scan.
    pub mods: Signal<Vec<crate::api::mod_manager::InstalledMod>>,
    /// True while scanning mods or fetching from Nexus.
    pub loading: Signal<bool>,
    /// Error message to display in the UI, if any.
    pub error: Signal<Option<String>>,
    /// True if we've checked Premium status this session.
    pub is_premium: Signal<Option<bool>>,
}