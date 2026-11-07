//! Runs commands with no UI, for checking USD files against other tools. Each line of the file is a JSON array:
//! `["polyCube", {"name": "box"}]`. Prints each result, and carries on after an error.

use aztek_scene::session::Session;

fn main() {
    let path = std::env::args().nth(1).expect("usage: usd_script commands.jsonl");
    let text = std::fs::read_to_string(&path).expect("can't read the file");
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).expect("bad JSON");
        let name = v[0].as_str().expect("command name");
        match s.run(name, v[1].clone()) {
            Ok(r) => println!("{name}: {r}"),
            Err(e) => println!("{name}: ERROR {e}"),
        }
    }
}
