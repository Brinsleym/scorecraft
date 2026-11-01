//! A score's rendered pages: cache hits for page turns, renders on a miss, and background
//! pre-rendering of the pages a turn will show next (architecture §7).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use scorecraft_pdf::{Bitmap, Document, PdfError, Tile};

use crate::ink::{compose, page_to_bitmap};
use crate::{CacheStats, Filter, PageInk, Region, TileCache, TileKey, ZoomBucket};

/// Side of a zoom tile in device pixels.
pub const TILE_SIDE: u32 = 512;

/// The pages to pre-render around `current` in a score of `count` pages: the current page, then
/// the next, then the previous. The next setlist piece's first page belongs to that piece's own
/// [`ScoreView`].
pub fn neighbours(current: usize, count: usize) -> Vec<usize> {
    let candidates = [
        Some(current),
        current.checked_add(1),
        current.checked_sub(1),
    ];
    candidates
        .into_iter()
        .flatten()
        .filter(|&p| p < count)
        .collect()
}

/// An open score's renderer and bitmap cache. Natively, [`ScoreView::start_worker`] adds a
/// background thread that renders [`ScoreView::prefetch`] requests; without one (the web build,
/// tests) [`ScoreView::run_prefetch`] renders them on the caller's thread.
pub struct ScoreView {
    doc: Document,
    filter: Filter,
    shared: Arc<Shared>,
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<std::thread::JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    /// Signals the worker: a job was queued or the view is closing.
    work: Condvar,
    /// Signals waiters: the queue drained and the worker is idle.
    idle: Condvar,
}

struct State {
    cache: TileCache,
    queue: VecDeque<TileKey>,
    /// The ink each page carries, set by the engine; pages without an entry have none.
    ink: HashMap<usize, PageInk>,
    /// The worker is rendering a request (native only; the web build has no worker).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    busy: bool,
    closing: bool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A render that panicked on the worker cannot leave the cache half-updated (inserts are
        // single calls), so a poisoned lock is still safe to use.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ScoreView {
    /// A view of `doc` keeping at most `budget` bytes of bitmaps.
    pub fn new(doc: Document, budget: usize) -> ScoreView {
        let doc = if doc.colors().is_some() {
            doc.fork(None)
        } else {
            doc
        };
        let state = State {
            cache: TileCache::new(budget),
            queue: VecDeque::new(),
            ink: HashMap::new(),
            busy: false,
            closing: false,
        };
        ScoreView {
            doc,
            filter: Filter::Normal,
            shared: Arc::new(Shared {
                state: Mutex::new(state),
                work: Condvar::new(),
                idle: Condvar::new(),
            }),
            #[cfg(not(target_arch = "wasm32"))]
            worker: None,
        }
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn filter(&self) -> Filter {
        self.filter
    }

    /// Switches the display filter. Bitmaps already cached for other filters stay cached, so
    /// switching back is instant.
    pub fn set_filter(&mut self, filter: Filter) {
        if filter != self.filter {
            self.doc = self.doc.fork(filter.colors());
            self.filter = filter;
        }
    }

    /// Page `page` at `scale` (snapped to its [`ZoomBucket`]) with the current filter: a cache
    /// hit, or rendered now and cached.
    pub fn page(&mut self, page: usize, scale: f32) -> Result<Arc<Bitmap>, PdfError> {
        let key = self.key(page, scale, Region::Page)?;
        if let Some(hit) = self.shared.lock().cache.get(&key) {
            return Ok(hit);
        }
        let bitmap = Arc::new(self.doc.render(page, key.zoom.scale())?);
        self.shared.lock().cache.insert(key, bitmap.clone());
        Ok(bitmap)
    }

    /// Sets the ink page `page` carries (`None`: no ink). Inked bitmaps of its old ink stay in the
    /// cache until evicted, so undo is instant too.
    pub fn set_ink(&self, page: usize, ink: Option<PageInk>) {
        let mut state = self.shared.lock();
        match ink {
            Some(i) if !i.marks.is_empty() => {
                state.ink.insert(page, i);
            }
            _ => {
                state.ink.remove(&page);
            }
        }
    }

    /// The [`PageInk::key`] of the ink page `page` carries, if any.
    pub fn ink_key(&self, page: usize) -> Option<u64> {
        self.shared.lock().ink.get(&page).map(|i| i.key)
    }

    /// Page `page` at `scale` with its ink drawn on: a cache hit, or drawn now (and rendered now
    /// if needed) and cached. Pages without ink are [`ScoreView::page`].
    pub fn inked_page(&mut self, page: usize, scale: f32) -> Result<Arc<Bitmap>, PdfError> {
        let base = self.page(page, scale)?;
        let key = self.key(page, scale, Region::Page)?;
        let Some((inked_key, ink)) = ink_job(&self.shared.lock(), key) else {
            return Ok(base);
        };
        if let Some(hit) = self.shared.lock().cache.get(&inked_key) {
            return Ok(hit);
        }
        let bitmap = draw_ink(&self.doc, key.page, key.filter, &base, &ink);
        self.shared.lock().cache.insert(inked_key, bitmap.clone());
        Ok(bitmap)
    }

    /// Whether page `page` at `scale` is cached with its ink (or has none), so
    /// [`ScoreView::inked_page`] returns without drawing.
    pub fn is_inked_cached(&self, page: usize, scale: f32) -> bool {
        let Ok(key) = self.key(page, scale, Region::Page) else {
            return false;
        };
        let state = self.shared.lock();
        state.cache.contains(&key)
            && ink_job(&state, key).is_none_or(|(k, _)| state.cache.contains(&k))
    }

    /// Columns and rows of [`TILE_SIDE`] tiles covering page `page` at `scale`.
    pub fn tile_grid(&self, page: usize, scale: f32) -> Result<(u32, u32), PdfError> {
        let zoom = ZoomBucket::of(scale).ok_or(PdfError::BadScale)?;
        let (w, h) = self.doc.render_size(page, zoom.scale())?;
        Ok((w.div_ceil(TILE_SIDE), h.div_ceil(TILE_SIDE)))
    }

    /// Tile (`col`, `row`) of page `page` at `scale`. Edge tiles are cut to the page.
    pub fn tile(
        &mut self,
        page: usize,
        scale: f32,
        col: u32,
        row: u32,
    ) -> Result<Arc<Bitmap>, PdfError> {
        let key = self.key(page, scale, Region::Tile { col, row })?;
        if let Some(hit) = self.shared.lock().cache.get(&key) {
            return Ok(hit);
        }
        let (w, h) = self.doc.render_size(page, key.zoom.scale())?;
        let x = col
            .checked_mul(TILE_SIDE)
            .filter(|&x| x < w)
            .ok_or(PdfError::BadTile)?;
        let y = row
            .checked_mul(TILE_SIDE)
            .filter(|&y| y < h)
            .ok_or(PdfError::BadTile)?;
        let tile = Tile {
            x,
            y,
            w: TILE_SIDE.min(w - x),
            h: TILE_SIDE.min(h - y),
        };
        let bitmap = Arc::new(self.doc.render_tile(page, key.zoom.scale(), tile)?);
        self.shared.lock().cache.insert(key, bitmap.clone());
        Ok(bitmap)
    }

    /// Replaces the pre-render queue with `pages` at `scale` and the current filter, in order.
    /// Pages already cached or out of range are skipped. Earlier requests that have not started
    /// are dropped, so fast turning never builds a backlog.
    pub fn prefetch(&self, pages: &[usize], scale: f32) {
        let each: Vec<(usize, f32)> = pages.iter().map(|&p| (p, scale)).collect();
        self.prefetch_each(&each);
    }

    /// Like [`ScoreView::prefetch`], with a scale per page (facing pages of different sizes
    /// render at different scales). Requests with a scale that isn't a positive number are
    /// skipped.
    pub fn prefetch_each(&self, pages: &[(usize, f32)]) {
        let count = self.doc.page_count();
        let mut state = self.shared.lock();
        state.queue.clear();
        for &(page, scale) in pages.iter().filter(|&&(p, _)| p < count) {
            let Some(zoom) = ZoomBucket::of(scale) else {
                continue;
            };
            let key = TileKey {
                page,
                zoom,
                filter: self.filter,
                region: Region::Page,
            };
            if needs_work(&state, key) && !state.queue.contains(&key) {
                state.queue.push_back(key);
            }
        }
        drop(state);
        self.shared.work.notify_one();
    }

    /// Pre-render requests not yet started.
    pub fn pending(&self) -> usize {
        self.shared.lock().queue.len()
    }

    /// Renders up to `max` queued pre-render requests on this thread; returns how many it
    /// rendered. Requests for another filter are dropped.
    pub fn run_prefetch(&mut self, max: usize) -> usize {
        let mut done = 0;
        while done < max {
            let Some(key) = self.shared.lock().queue.pop_front() else {
                break;
            };
            if key.filter != self.filter {
                continue;
            }
            let cached = self.shared.lock().cache.get(&key);
            let base = match cached {
                Some(b) => Some(b),
                None => self
                    .doc
                    .render(key.page, key.zoom.scale())
                    .ok()
                    .map(Arc::new)
                    .inspect(|b| self.shared.lock().cache.insert(key, b.clone())),
            };
            if let Some(base) = base {
                let job = ink_job(&self.shared.lock(), key);
                if let Some((inked_key, ink)) = job {
                    let bitmap = draw_ink(&self.doc, key.page, key.filter, &base, &ink);
                    self.shared.lock().cache.insert(inked_key, bitmap);
                }
            }
            done += 1;
        }
        self.shared.idle.notify_all();
        done
    }

    /// Starts the background pre-render thread, which owns its own [`Document`] handle. Does
    /// nothing if it is already running.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_worker(&mut self) -> std::io::Result<()> {
        if self.worker.is_some() {
            return Ok(());
        }
        let shared = self.shared.clone();
        let filter = self.filter;
        let doc = self.doc.fork(filter.colors());
        let handle = std::thread::Builder::new()
            .name("scorecraft-prefetch".into())
            .spawn(move || worker(&shared, doc, filter))?;
        self.worker = Some(handle);
        Ok(())
    }

    /// Waits until the background worker has drained the pre-render queue, up to `timeout`.
    /// Returns whether it became idle. Native only: the web build has no worker and no clock to
    /// wait on, and renders with [`ScoreView::run_prefetch`] instead.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait_for_prefetch(&self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now().checked_add(timeout);
        let mut state = self.shared.lock();
        while state.busy || !state.queue.is_empty() {
            let left = match deadline {
                Some(d) => d.saturating_duration_since(std::time::Instant::now()),
                None => timeout,
            };
            if left.is_zero() {
                return false;
            }
            state = match self.shared.idle.wait_timeout(state, left) {
                Ok((s, _)) => s,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        true
    }

    /// Whether page `page` at `scale` with the current filter is cached.
    pub fn is_cached(&self, page: usize, scale: f32) -> bool {
        self.key(page, scale, Region::Page)
            .is_ok_and(|k| self.shared.lock().cache.contains(&k))
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.shared.lock().cache.stats()
    }

    /// Bytes of bitmaps cached.
    pub fn cached_bytes(&self) -> usize {
        self.shared.lock().cache.bytes()
    }

    /// Changes the cache budget, evicting the oldest bitmaps if needed.
    pub fn set_budget(&self, budget: usize) {
        self.shared.lock().cache.set_budget(budget);
    }

    fn key(&self, page: usize, scale: f32, region: Region) -> Result<TileKey, PdfError> {
        self.doc.page(page)?;
        let zoom = ZoomBucket::of(scale).ok_or(PdfError::BadScale)?;
        Ok(TileKey {
            page,
            zoom,
            filter: self.filter,
            region,
        })
    }
}

impl Drop for ScoreView {
    fn drop(&mut self) {
        {
            let mut state = self.shared.lock();
            state.closing = true;
            state.queue.clear();
        }
        self.shared.work.notify_all();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(handle) = self.worker.take() {
            // A worker that panicked has nothing left to clean up.
            let _ = handle.join();
        }
    }
}

impl std::fmt::Debug for ScoreView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScoreView")
            .field("pages", &self.doc.page_count())
            .field("filter", &self.filter)
            .finish_non_exhaustive()
    }
}

/// The pre-render loop: take a request, render it without holding the lock, cache it.
#[cfg(not(target_arch = "wasm32"))]
fn worker(shared: &Shared, mut doc: Document, mut filter: Filter) {
    loop {
        let key = {
            let mut state = shared.lock();
            loop {
                if state.closing {
                    return;
                }
                if let Some(key) = state.queue.pop_front() {
                    if needs_work(&state, key) {
                        state.busy = true;
                        break key;
                    }
                    continue;
                }
                shared.idle.notify_all();
                state = shared
                    .work
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        if key.filter != filter {
            doc = doc.fork(key.filter.colors());
            filter = key.filter;
        }
        // A failed render is not cached: the page is rendered again, and the error reported,
        // when it is shown.
        let cached = shared.lock().cache.get(&key);
        let base = match cached {
            Some(b) => Some(b),
            None => doc.render(key.page, key.zoom.scale()).ok().map(Arc::new),
        };
        let mut state = shared.lock();
        if let Some(base) = base {
            state.cache.insert(key, base.clone());
            // Then the page's ink, so a turn to it never waits on drawing either.
            if let Some((inked_key, ink)) = ink_job(&state, key)
                && !state.cache.contains(&inked_key)
            {
                drop(state);
                let bitmap = draw_ink(&doc, key.page, key.filter, &base, &ink);
                state = shared.lock();
                state.cache.insert(inked_key, bitmap);
            }
        }
        state.busy = false;
        if state.queue.is_empty() {
            shared.idle.notify_all();
        }
    }
}

/// The inked-page key and ink for plain page `key`, if its page carries ink.
fn ink_job(state: &State, key: TileKey) -> Option<(TileKey, PageInk)> {
    let ink = state.ink.get(&key.page)?;
    Some((
        TileKey {
            region: Region::Inked { ink: ink.key },
            ..key
        },
        ink.clone(),
    ))
}

/// Whether pre-rendering `key` has anything to do: render the page, or draw its ink.
fn needs_work(state: &State, key: TileKey) -> bool {
    !state.cache.contains(&key)
        || ink_job(state, key).is_some_and(|(k, _)| !state.cache.contains(&k))
}

/// `base` (page `page` of `doc`) with `ink` drawn on, under `filter`.
fn draw_ink(
    doc: &Document,
    page: usize,
    filter: Filter,
    base: &Arc<Bitmap>,
    ink: &PageInk,
) -> Arc<Bitmap> {
    let marks: Vec<&scorecraft_doc::AnnotationKind> = ink.marks.iter().collect();
    let Some(m) = doc.page(page).ok().and_then(|g| page_to_bitmap(g, base)) else {
        return base.clone();
    };
    let Some(mut out) = compose(base, m, &marks, filter, &ink.curve) else {
        return base.clone();
    };
    // Snips copy another page's bars: render that page at this page's scale (PAGE-7).
    let snips: Vec<&scorecraft_doc::Snip> = marks
        .iter()
        .filter_map(|k| match k {
            scorecraft_doc::AnnotationKind::Snip(s) => Some(s),
            _ => None,
        })
        .collect();
    if !snips.is_empty() {
        let scale = m
            .coeffs()
            .iter()
            .take(4)
            .map(|c| c.abs())
            .fold(0.0, f64::max) as f32;
        let mut src_doc = doc.fork(doc.colors());
        for s in snips {
            let Ok(src_page) = usize::try_from(s.from.page.0) else {
                continue;
            };
            let Ok(src) = src_doc.render(src_page, scale) else {
                continue;
            };
            if let Some(src_m) = src_doc
                .page(src_page)
                .ok()
                .and_then(|g| page_to_bitmap(g, &src))
            {
                crate::ink::draw_snip(&mut out, m, &src, src_m, s);
            }
        }
    }
    Arc::new(out)
}
