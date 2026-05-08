use dioxus::prelude::*;
use crate::Route;
use crate::components::{navbar::Navbar, footer::Footer};



#[component]
pub fn Layout() -> Element {
    rsx! {
        div { class: "min-h-screen bg-white text-gray-900 flex flex-col",
            // Skip to main content (visually hidden, shown on keyboard focus)
            a {
                href: "#main-content",
                class: "sr-only focus:not-sr-only focus:absolute focus:top-4 focus:left-4 focus:z-50 focus:px-4 focus:py-2 focus:bg-blue-600 focus:text-white focus:rounded-lg focus:font-medium",
                "Skip to main content"
            }
            // Header
            header { Navbar {} }
            // Main Content Area
            main {
                id: "main-content",
                tabindex: "-1",
                class: "flex-1 px-4 sm:px-6 lg:px-8 py-8",
                div { class: "", Outlet::<Route> {} }
            }

            // Footer
            Footer {}
        }
    }
} 