//! Prints every registered node type, one per line, for the reference capture kit:
//! `cargo run -p aztek_host --example list_node_types > tools/reference_capture/node_types.txt`

fn main() {
    let _c = aztek_host::Core::new();
    let mut v: Vec<String> = aztek_scene::node::types::all().iter().map(|t| t.name.clone()).collect();
    v.sort();
    v.dedup();
    for n in v {
        println!("{n}");
    }
}
