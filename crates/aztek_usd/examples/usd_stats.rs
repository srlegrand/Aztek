//! How long a stage takes to read for drawing: `usd_stats file.usd`. For stress tests with no display.

use std::time::{Duration, Instant};

use aztek_usd::draw::{Filter, Scene};

fn main() {
    let file = std::env::args().nth(1).expect("usage: usd_stats file.usd");
    let t0 = Instant::now();
    let e = aztek_usd::stages::open_entry(&file, "", true).expect("open");
    println!("open: {:?}", t0.elapsed());
    let mut scene = Scene::default();
    let filter = Filter::default();
    let t1 = Instant::now();
    let mut slices = 0;
    let mut longest = Duration::ZERO;
    loop {
        let s = Instant::now();
        let done = scene.update(&e.stage, e.version.get(), &e.changes.borrow(), &filter, 1.0, Duration::from_millis(10));
        longest = longest.max(s.elapsed());
        slices += 1;
        if done {
            break;
        }
    }
    let tris: usize = scene.geoms.iter().map(|g| g.indices.len() / 3).sum();
    let drawn: usize = scene.items.iter().map(|i| scene.geoms[i.geom].indices.len() / 3).sum();
    println!("read: {:?} in {slices} slices, longest {:?}", t1.elapsed(), longest);
    println!("prims visited {}, items {}, geoms {}, looks {}, tris stored {tris}, tris drawn {drawn}", scene.prims_visited, scene.items.len(), scene.geoms.len(), scene.looks.len());
    let (lo, hi) = scene.bounds(None).unwrap_or_default();
    println!("bounds {lo} {hi}");
    let t2 = Instant::now();
    let hit = scene.raycast((lo + hi) * 0.5 + glam::DVec3::new(0.0, 100.0, 0.0), glam::DVec3::NEG_Y);
    println!("raycast {:?}: {:?}", t2.elapsed(), hit.map(|h| h.0));
}
