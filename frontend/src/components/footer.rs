use dioxus::prelude::*;
use crate::routes::Route;


#[component]
pub fn Footer() -> Element {
    rsx! {
        footer {
            class: "  w-full bg-gray-50 text-gray-700 py-6 border-t border-gray-200",
            "aria-label": "Site footer",
            div { class: " mx-auto px-6",
                div { class: "grid grid-cols-1 md:grid-cols-4 gap-8 mb-8" }
                p { "2026 © Stardew Mod Manager. All rights reserved." }
            }
        
        }
    }
        
        }
 
