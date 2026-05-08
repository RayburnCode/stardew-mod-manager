
   // client/components/layout/app_layout.rs
use dioxus::prelude::*;


#[component]
pub fn NotFound(route: Vec<String>) -> Element {
    let attempted_path = if route.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", route.join("/"))
    };

    rsx! {
        document::Title { "Page Not Found – Stardew Mod Manager" }
        document::Meta { name: "robots", content: "noindex, follow" }

        div {
            class: "min-h-screen flex flex-col",
            style: "
                font-family: 'Berkeley Mono', 'Fira Code', 'JetBrains Mono', monospace;
                background: #0f1117;
                color: #e8e6df;
            ",

            div { style: "
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    padding: 0 20px;
                    height: 52px;
                    border-bottom: 1px solid #1e2130;
                    background: #0c0e14;
                ",
                span { style: "
                        font-size: 13px;
                        font-weight: 600;
                        letter-spacing: 0.08em;
                        color: #7ec8a4;
                        text-transform: uppercase;
                    ",
                    "Stardew Mod Manager"
                }
                span { class: "text-xs text-[#9ca3af]", "404" }
            }

            div { class: "flex-1 grid place-items-center px-6 py-20",
                div { class: "w-full max-w-2xl rounded-lg border border-[#1e2130] bg-[#0c0e14] p-8 text-center",

                    p { class: "text-sm font-semibold tracking-[0.08em] uppercase text-[#7ec8a4]",
                        "Page Not Found"
                    }
                    h1 { class: "mt-3 text-3xl font-semibold text-[#e8e6df] sm:text-5xl",
                        "This path does not exist"
                    }
                    p { class: "mt-4 text-sm text-[#b0b8c7]",
                        "The requested route could not be resolved in the app router."
                    }
                    p { class: "mt-2 text-xs text-[#9ca3af]", "Attempted: {attempted_path}" }

                    div { class: "mt-8 flex items-center justify-center gap-3",
                        a {
                            class: "rounded-md bg-[#7ec8a4] px-4 py-2 text-sm font-semibold text-[#0c0e14] hover:opacity-90",
                            href: "/",
                            "Go Home"
                        }
                    }
                }
            }
        }
    }
}
   
   
   