//! Writes docs/ENVIRONMENT.md from the list of variables: cargo run -p aztek_host --example gen_environment

fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ENVIRONMENT.md");
    std::fs::write(&path, aztek_core::env::markdown()).expect("write docs/ENVIRONMENT.md");
    println!("wrote {}", path.display());
}
