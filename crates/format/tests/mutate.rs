//! A deterministic stand-in for a fuzz target of the library loader (gap G3): a stored library
//! has each of its files damaged in many ways (bytes flipped, cut short, emptied, garbage, deep
//! nesting, huge numbers), one at a time and several together, and opening it must never
//! panic. It either refuses with an error or opens, skipping what it can't read, and the library
//! it opens passes the model's own checks.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use scorecraft_format::{MemStorage, Store};
use scorecraft_testkit::library::{NOW_MS, sample_library};

/// A tiny deterministic generator (xorshift), so every run damages the same way.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// `data` damaged one of several ways.
fn damage(data: &[u8], rng: &mut Rng) -> Vec<u8> {
    let mut d = data.to_vec();
    match rng.below(9) {
        0 => {
            for _ in 0..=rng.below(8) {
                let i = rng.below(d.len());
                if let Some(b) = d.get_mut(i) {
                    *b ^= 1 << rng.below(8);
                }
            }
        }
        1 => d.truncate(rng.below(d.len())),
        2 => d.clear(),
        3 => d = (0..rng.below(512)).map(|_| rng.next() as u8).collect(),
        4 => d = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000)).into_bytes(),
        5 => d = b"{\"version\": 1e999999, \"id\": -1}".to_vec(),
        6 => {
            // Swap two JSON delimiters.
            let (i, j) = (rng.below(d.len()), rng.below(d.len()));
            if i < d.len() && j < d.len() {
                d.swap(i, j);
            }
        }
        7 => d.extend(std::iter::repeat_n(b'9', 4096)),
        _ => {
            let i = rng.below(d.len());
            d.insert(i.min(d.len()), 0xff);
        }
    }
    d
}

fn check_opens_or_refuses(mem: &MemStorage) {
    // Refusing is fine; checking what it opened never panics either (Verify & Repair works
    // from this).
    if let Ok((_, lib, _)) = Store::open(mem.clone()) {
        let _ = lib.problems();
    }
}

#[test]
fn a_damaged_library_never_panics_the_loader() {
    let lib = sample_library(3, 5);
    let clean = MemStorage::new();
    let (mut store, _) = Store::create(clean.clone(), lib.id, NOW_MS).unwrap();
    store.save_all(&lib).unwrap();
    let files = clean.files();
    let names: Vec<&String> = files.keys().collect();
    let mut rng = Rng(0x5eed_cafe_f00d_d00d);
    // Every file, several damages each.
    for name in &names {
        for _ in 0..12 {
            let mem = MemStorage::new();
            for (k, v) in &files {
                mem.plant(k, v);
            }
            mem.plant(name, &damage(&files[*name], &mut rng));
            check_opens_or_refuses(&mem);
        }
    }
    // Several files at once.
    for _ in 0..200 {
        let mem = MemStorage::new();
        for (k, v) in &files {
            let v = if rng.below(3) == 0 {
                damage(v, &mut rng)
            } else {
                v.clone()
            };
            mem.plant(k, &v);
        }
        // Stray files where the store looks.
        if rng.below(4) == 0 {
            mem.plant("journal/stray.json", &damage(b"{}", &mut rng));
        }
        check_opens_or_refuses(&mem);
    }
}
