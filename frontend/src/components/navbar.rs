use dioxus::prelude::*;
use crate::routes::Route;

#[component]
pub fn Navbar(children: Element) -> Element {
        let current_route = use_route::<Route>();
    let mut is_mobile_menu_open = use_signal(|| false);

    // Helper function to determine active class
    fn active_class(route: &Route, current_route: &Route, class: &str) -> String {
        if route == current_route {
            format!("{} text-blue-600 transition-colors font-medium", class)
        } else {
            class.to_string()
        }
    }

    // Helper function for mobile active class
    fn mobile_active_class(route: &Route, current_route: &Route, class: &str) -> String {
        if route == current_route {
            format!("{} text-blue-600 font-medium border-l-4 border-blue-600 bg-blue-50", class)
        } else {
            class.to_string()
        }
    }
    // Precompute aria-current values
    let aria_home = if current_route == (Route::Home {}) { "page" } else { "false" };
    let menu_expanded = if is_mobile_menu_open() { "true" } else { "false" };

    rsx! {
        nav {
            class: "w-full bg-white text-gray-900 px-6 py-4 border-b border-gray-200 flex items-center justify-between shadow-sm",
            "aria-label": "Main navigation",
            div { class: "flex items-center gap-2",
                Link {
                    to: Route::Home {},
                    "aria-label": "DSCR Connect – go to home page",
                    svg {
                        height: "24",
                        view_box: "0 0 24 24",
                        width: "24",
                        xmlns: "http://www.w3.org/2000/svg",
                        "aria-hidden": "true",
                        "focusable": "false",
                        path {
                            d: "m6.5 17.5l8.25-5.5L6.5 6.5l1-1.5L18 12L7.5 19z",
                            fill: "#1e40af",
                            fill_rule: "evenodd",
                        }
                    }
                }
                Link {
                    to: Route::Home {},
                    tabindex: "-1",
                    "aria-hidden": "true",
                    span { class: "font-bold cursor-pointer text-xl tracking-tight text-blue-600",
                        "Mod Manager"
                    }
                }
            }
            // Desktop navigation menu
            div { class: "hidden md:flex gap-6 items-center",
                Link {
                    to: Route::Home {},
                    class: active_class(
                        &Route::Home {},
                        &current_route,
                        "text-gray-700 hover:text-blue-600 px-1 py-2 text-sm font-medium transition-colors",
                    ),
                    "aria-current": aria_home,
                    "What is it?"
                }
            
            }

            // Mobile hamburger menu button
            button {
                class: "md:hidden inline-flex items-center justify-center p-2 rounded-md text-gray-700 hover:text-blue-600 hover:bg-gray-100 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-blue-600 transition-colors",
                "aria-expanded": menu_expanded,
                "aria-controls": "mobile-menu",
                "aria-label": "Toggle navigation menu",
                onclick: move |_| {
                    is_mobile_menu_open.set(!is_mobile_menu_open());
                },
                // Hamburger icon
                if !is_mobile_menu_open() {
                    svg {
                        class: "h-6 w-6",
                        fill: "none",
                        "viewBox": "0 0 24 24",
                        stroke: "currentColor",
                        "aria-hidden": "true",
                        "focusable": "false",
                        path {
                            "stroke-linecap": "round",
                            "stroke-linejoin": "round",
                            "stroke-width": "2",
                            d: "M4 6h16M4 12h16M4 18h16",
                        }
                    }
                } else {
                    // Close icon
                    svg {
                        class: "h-6 w-6",
                        fill: "none",
                        "viewBox": "0 0 24 24",
                        stroke: "currentColor",
                        "aria-hidden": "true",
                        "focusable": "false",
                        path {
                            "stroke-linecap": "round",
                            "stroke-linejoin": "round",
                            "stroke-width": "2",
                            d: "M6 18L18 6M6 6l12 12",
                        }
                    }
                }
            }
        }

        // Mobile menu
        if is_mobile_menu_open() {
            div {
                id: "mobile-menu",
                class: "md:hidden border-t border-gray-200 bg-white",
                "aria-label": "Mobile navigation",
                div { class: "px-2 pt-2 pb-3 space-y-1",
                    Link {
                        to: Route::Home {},
                        class: mobile_active_class(
                            &Route::Home {},
                            &current_route,
                            "block px-3 py-2 text-base font-medium text-gray-700 hover:text-blue-600 hover:bg-gray-100 transition-colors",
                        ),
                        "aria-current": aria_home,
                        onclick: move |_| is_mobile_menu_open.set(false),
                        "Home"
                    }
                    Link {
                        to: Route::Home {},
                        class: mobile_active_class(
                            &Route::Home {},
                            &current_route,
                            "block px-3 py-2 text-base font-medium text-gray-700 hover:text-blue-600 hover:bg-gray-100 transition-colors",
                        ),
                        "aria-current": aria_home,
                        onclick: move |_| is_mobile_menu_open.set(false),
                        "What is it?"
                    }
                
                }
            }
        }
    }}
                        