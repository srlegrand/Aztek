//! USD in Aztek, on the native `openusd` crate: import and export, the stage in the DAG, prims, layers, and translation
//! between prims and nodes. Commands take the names and flags of the reference's USD plug-in and of the Animal Logic USD plug-in.

pub mod draw;
pub mod export;
pub mod extras;
pub mod file_cmd;
mod export_geo;
mod export_mat;
mod export_skel;
pub mod import;
mod import_shade;
mod import_skel;
pub mod mesh_read;
pub mod layers;
pub mod out;
pub mod panels;
pub mod prim_cmds;
pub mod prims;
pub mod proxy;
pub mod py_api;
pub mod stages;
pub mod translate;
pub mod util;

use aztek_scene::session::Session;

/// Adds USD's commands and nodes to a session.
pub fn install(s: &mut Session) {
    export::register(&mut s.commands);
    import::register(&mut s.commands);
    file_cmd::register(&mut s.commands);
    proxy::register(&mut s.commands);
    prim_cmds::register(&mut s.commands);
    layers::register(&mut s.commands);
    translate::register(&mut s.commands);
    py_api::register(&mut s.commands);
    extras::register(&mut s.commands);
    prims::install_wrappers(&mut s.commands);
}
