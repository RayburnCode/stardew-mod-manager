use dioxus::prelude::*;
use crate::views::{Home, NotFound, ModList, Settings};
use crate::components::Layout;


#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    // #[layout(Layout)]
    #[route("/")]
    Home {},

    #[route("/mod-list")]
    ModList {},
    #[route("/settings")]
    Settings {},


    #[route("/:..route")]
    NotFound { route: Vec<String> },
}