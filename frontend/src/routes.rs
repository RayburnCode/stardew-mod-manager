use dioxus::prelude::*;
use crate::views::{Home, NotFound};
use crate::components::Layout;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    Home {},


    #[route("/:..route")]
    NotFound { route: Vec<String> },
}