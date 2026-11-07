//! Writes docs/COMMANDS.md from the command registry: cargo run -p aztek_host --example gen_commands

use aztek_host::Core;
use std::fmt::Write as _;

fn main() {
    let c = Core::new();
    let mut names: Vec<_> = c.s.commands.all().filter(|d| !d.scripted).collect();
    names.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = String::new();
    let _ = writeln!(out, "# Command reference\n");
    let _ = writeln!(out, "Generated from the command registry by `cargo run -p aztek_host --example gen_commands`; do not edit by hand.");
    let _ = writeln!(out, "Commands take the reference's flags, long or short, in MEL and in Python (`aztek.cmds` or `maya.cmds`).\n");
    let _ = writeln!(out, "{} commands.\n", names.len());
    for d in names {
        let _ = writeln!(out, "## {}\n", d.name);
        let _ = writeln!(out, "{}\n", d.doc.split_whitespace().collect::<Vec<_>>().join(" "));
        if !d.positional_doc.is_empty() {
            let _ = writeln!(out, "Arguments: {}\n", d.positional_doc);
        }
        let flags: Vec<_> = d.flags.iter().filter(|f| f.name != "queried").collect();
        if !flags.is_empty() {
            let _ = writeln!(out, "| Flag | Short | Description |\n| --- | --- | --- |");
            for f in flags {
                let doc = f.doc.split_whitespace().collect::<Vec<_>>().join(" ").replace('|', "\\|");
                let _ = writeln!(out, "| `{}` | {} | {} |", f.name, if f.short_name.is_empty() { String::new() } else { format!("`{}`", f.short_name) }, doc);
            }
            let _ = writeln!(out);
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/COMMANDS.md");
    std::fs::write(&path, out).expect("write docs/COMMANDS.md");
    println!("wrote {}", path.display());
}
