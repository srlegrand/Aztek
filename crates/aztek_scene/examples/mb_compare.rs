//! Opens a scene saved both ways and writes what Aztek holds for each, to compare:
//! `cargo run -p aztek_scene --example mb_compare -- scene.ma scene.mb out_dir`. With one `.mb` argument, prints
//! the `.ma` text it is read as.

use aztek_scene::{ma_file, mb_file, Session};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [mb] = args.as_slice() {
        let c = mb_file::convert(&std::fs::read(mb).expect("read")).expect("convert");
        print!("{}", c.text);
        c.warnings.iter().for_each(|w| eprintln!("warning: {w}"));
        return;
    }
    let [ma, mb, out] = args.as_slice() else { panic!("usage: mb_compare scene.ma scene.mb out_dir") };
    std::fs::create_dir_all(out).expect("out dir");
    for (src, name) in [(ma, "from_ma.ma"), (mb, "from_mb.ma")] {
        let mut s = Session::new();
        let w = ma_file::open_scene(&mut s, src).expect("open");
        eprintln!("{src}: {} warning(s)", w.len());
        w.iter().take(20).for_each(|x| eprintln!("  {x}"));
        s.graph.evaluate();
        let path = format!("{out}/{name}");
        ma_file::write_file(&mut s, &path, ma_file::Format::Ma, None).expect("write");
    }
}
