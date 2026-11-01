//! A byte-budget LRU of rendered pages and tiles.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use scorecraft_pdf::Bitmap;

use crate::{Filter, ZoomBucket};

/// Which part of a page a cached bitmap covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// The whole page.
    Page,
    /// One [`crate::TILE_SIDE`]-pixel tile of the page, by column and row.
    Tile { col: u32, row: u32 },
    /// The whole page with its ink drawn on; `ink` is the [`crate::PageInk::key`] it was drawn
    /// from, so changed ink is a different entry.
    Inked { ink: u64 },
}

/// What a cached bitmap shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    /// 0-based page index.
    pub page: usize,
    pub zoom: ZoomBucket,
    pub filter: Filter,
    pub region: Region,
}

/// Counters for tests, the perf harness and diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
}

struct Entry {
    bitmap: Arc<Bitmap>,
    tick: u64,
}

/// Least-recently-used bitmaps within a byte budget. A bitmap larger than the whole budget is
/// returned to its caller but not kept.
pub struct TileCache {
    budget: usize,
    bytes: usize,
    clock: u64,
    entries: HashMap<TileKey, Entry>,
    by_age: BTreeMap<u64, TileKey>,
    stats: CacheStats,
}

impl TileCache {
    /// An empty cache holding at most `budget` bytes of pixels.
    pub fn new(budget: usize) -> TileCache {
        TileCache {
            budget,
            bytes: 0,
            clock: 0,
            entries: HashMap::new(),
            by_age: BTreeMap::new(),
            stats: CacheStats::default(),
        }
    }

    /// The bitmap for `key`, marking it most recently used.
    pub fn get(&mut self, key: &TileKey) -> Option<Arc<Bitmap>> {
        let tick = self.tick();
        let Some(entry) = self.entries.get_mut(key) else {
            self.stats.misses += 1;
            return None;
        };
        self.by_age.remove(&entry.tick);
        entry.tick = tick;
        self.by_age.insert(tick, *key);
        self.stats.hits += 1;
        Some(entry.bitmap.clone())
    }

    /// Whether `key` is cached, without touching its age or the counters.
    pub fn contains(&self, key: &TileKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Stores `bitmap` under `key` as most recently used, evicting the oldest entries to stay
    /// within the budget.
    pub fn insert(&mut self, key: TileKey, bitmap: Arc<Bitmap>) {
        self.remove(&key);
        let size = bitmap.byte_len();
        if size > self.budget {
            return;
        }
        while self.bytes.saturating_add(size) > self.budget {
            let Some((_, oldest)) = self.by_age.pop_first() else {
                break;
            };
            if let Some(gone) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(gone.bitmap.byte_len());
                self.stats.evictions += 1;
            }
        }
        let tick = self.tick();
        self.by_age.insert(tick, key);
        self.entries.insert(key, Entry { bitmap, tick });
        self.bytes = self.bytes.saturating_add(size);
        self.stats.inserts += 1;
    }

    /// Drops `key` if it is cached.
    pub fn remove(&mut self, key: &TileKey) {
        if let Some(gone) = self.entries.remove(key) {
            self.by_age.remove(&gone.tick);
            self.bytes = self.bytes.saturating_sub(gone.bitmap.byte_len());
        }
    }

    /// Drops everything, for example when the score's file changes.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.by_age.clear();
        self.bytes = 0;
    }

    /// Changes the budget, evicting the oldest entries if the cache is now over it.
    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
        while self.bytes > self.budget {
            let Some((_, oldest)) = self.by_age.pop_first() else {
                break;
            };
            if let Some(gone) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(gone.bitmap.byte_len());
                self.stats.evictions += 1;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bytes of pixels held.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(page: usize) -> TileKey {
        TileKey {
            page,
            zoom: ZoomBucket::of(1.0).unwrap(),
            filter: Filter::Normal,
            region: Region::Page,
        }
    }

    fn bmp(pixels: u32) -> Arc<Bitmap> {
        Arc::new(Bitmap {
            width: pixels,
            height: 1,
            words: vec![0; pixels as usize],
        })
    }

    #[test]
    fn evicts_least_recently_used_first() {
        let mut c = TileCache::new(300);
        c.insert(key(0), bmp(25));
        c.insert(key(1), bmp(25));
        c.insert(key(2), bmp(25));
        assert!(c.get(&key(0)).is_some());
        c.insert(key(3), bmp(25));
        assert_eq!(c.len(), 3);
        assert!(!c.contains(&key(1)), "page 1 was the oldest");
        assert!(c.contains(&key(0)) && c.contains(&key(2)) && c.contains(&key(3)));
        assert_eq!(c.bytes(), 300);
        assert_eq!(c.stats().evictions, 1);
    }

    #[test]
    fn oversize_bitmaps_are_not_kept() {
        let mut c = TileCache::new(100);
        c.insert(key(0), bmp(10));
        c.insert(key(1), bmp(26));
        assert!(!c.contains(&key(1)));
        assert!(
            c.contains(&key(0)),
            "an oversize insert must not flush the cache"
        );
    }

    #[test]
    fn reinserting_replaces_and_counts_bytes_once() {
        let mut c = TileCache::new(1000);
        c.insert(key(0), bmp(10));
        c.insert(key(0), bmp(20));
        assert_eq!((c.len(), c.bytes()), (1, 80));
        c.remove(&key(0));
        assert_eq!((c.len(), c.bytes()), (0, 0));
        assert!(c.is_empty());
    }

    #[test]
    fn shrinking_the_budget_evicts() {
        let mut c = TileCache::new(1000);
        for p in 0..5 {
            c.insert(key(p), bmp(50));
        }
        c.set_budget(450);
        assert_eq!(c.len(), 2);
        assert!(c.contains(&key(3)) && c.contains(&key(4)));
        assert_eq!(c.budget(), 450);
        c.clear();
        assert_eq!(c.bytes(), 0);
    }

    #[test]
    fn hits_and_misses_are_counted() {
        let mut c = TileCache::new(1000);
        assert!(c.get(&key(0)).is_none());
        c.insert(key(0), bmp(1));
        assert!(c.get(&key(0)).is_some());
        let s = c.stats();
        assert_eq!((s.hits, s.misses, s.inserts), (1, 1, 1));
    }
}
