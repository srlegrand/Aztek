//! The disk cache: every frame's point positions for each cached body, in one file.
//!
//! Layout, little endian:
//!   magic "AZDC", u32 version (1)
//!   then records until the end: f64 frame, u32 channel count, and per channel: u32 name length, the name in UTF-8,
//!   u32 point count, then x y z as f32 for each point.
//! Records may come in any order; a later record for the same frame replaces an earlier one. Append writes records at
//! the end, so a cache grows without being rewritten.

use std::collections::BTreeMap;
use std::io::{Read, Write};

use glam::DVec3;

pub const MAGIC: &[u8; 4] = b"AZDC";
pub const VERSION: u32 = 1;

/// Frames keyed by frame times 1000, rounded, so 1.5 and 1.500 are the same frame.
#[derive(Clone, Debug, Default)]
pub struct Cache {
    pub frames: BTreeMap<i64, Vec<(String, Vec<DVec3>)>>,
}

pub fn key(frame: f64) -> i64 {
    (frame * 1000.0).round() as i64
}

impl Cache {
    pub fn insert(&mut self, frame: f64, channels: Vec<(String, Vec<DVec3>)>) {
        self.frames.insert(key(frame), channels);
    }

    pub fn get(&self, frame: f64, channel: &str) -> Option<&[DVec3]> {
        self.frames.get(&key(frame))?.iter().find(|(n, _)| n == channel).map(|(_, p)| p.as_slice())
    }

    pub fn range(&self) -> Option<(f64, f64)> {
        Some((*self.frames.keys().next()? as f64 / 1000.0, *self.frames.keys().next_back()? as f64 / 1000.0))
    }

    fn record(out: &mut Vec<u8>, frame: f64, channels: &[(String, Vec<DVec3>)]) {
        out.extend_from_slice(&frame.to_le_bytes());
        out.extend_from_slice(&(channels.len() as u32).to_le_bytes());
        for (name, pts) in channels {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&(pts.len() as u32).to_le_bytes());
            for p in pts {
                for c in [p.x, p.y, p.z] {
                    out.extend_from_slice(&(c as f32).to_le_bytes());
                }
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        for (k, ch) in &self.frames {
            Self::record(&mut out, *k as f64 / 1000.0, ch);
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Result<Cache, String> {
        if b.len() < 8 || &b[0..4] != MAGIC {
            return Err("not a cache file".into());
        }
        let mut at = 8;
        let mut take = |n: usize| -> Result<&[u8], String> {
            if at + n > b.len() {
                return Err("cache file cut short".into());
            }
            let s = &b[at..at + n];
            at += n;
            Ok(s)
        };
        let mut c = Cache::default();
        loop {
            let Ok(f) = take(8) else { break };
            let frame = f64::from_le_bytes(f.try_into().unwrap());
            let count = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
            let mut channels = Vec::with_capacity(count.min(4096));
            for _ in 0..count {
                let len = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
                let name = String::from_utf8_lossy(take(len)?).into_owned();
                let n = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
                let raw = take(n * 12)?;
                let pts = raw
                    .chunks_exact(12)
                    .map(|q| {
                        let f = |k: usize| f32::from_le_bytes(q[k..k + 4].try_into().unwrap()) as f64;
                        DVec3::new(f(0), f(4), f(8))
                    })
                    .collect();
                channels.push((name, pts));
            }
            c.frames.insert(key(frame), channels);
        }
        Ok(c)
    }

    pub fn write(&self, path: &str) -> Result<(), String> {
        std::fs::write(path, self.to_bytes()).map_err(|e| format!("{path}: {e}"))
    }

    pub fn read(path: &str) -> Result<Cache, String> {
        let mut b = Vec::new();
        std::fs::File::open(path).and_then(|mut f| f.read_to_end(&mut b)).map_err(|e| format!("{path}: {e}"))?;
        Cache::from_bytes(&b)
    }

    /// Adds frames to the end of a cache file, making it when missing.
    pub fn append(path: &str, frame: f64, channels: &[(String, Vec<DVec3>)]) -> Result<(), String> {
        let exists = std::path::Path::new(path).exists();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).map_err(|e| format!("{path}: {e}"))?;
        let mut out = Vec::new();
        if !exists {
            out.extend_from_slice(MAGIC);
            out.extend_from_slice(&VERSION.to_le_bytes());
        }
        Self::record(&mut out, frame, channels);
        f.write_all(&out).map_err(|e| format!("{path}: {e}"))
    }
}
