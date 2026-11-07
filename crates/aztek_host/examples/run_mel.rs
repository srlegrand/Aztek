//! Runs a MEL file with no window, for scripts that only build and save scenes:
//! `cargo run -p aztek_host --example run_mel -- examples/tools/make_examples.mel`

use aztek_host::Core;

fn main() {
    let path = std::env::args().nth(1).expect("usage: run_mel file.mel");
    let src = std::fs::read_to_string(&path).expect("read");
    let mut c = Core::new();
    aztek_scene::cameras::make_defaults(&mut c.s.graph);
    match c.mel_checked(&src) {
        Ok(_) => eprintln!("ran {path}"),
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    }
    for l in c.log.iter().filter(|l| matches!(l.kind as i32, 2 | 4)) {
        eprintln!("{}", l.text);
    }
}
