//! `ScoreView` against synthetic scores: cache hits, tiles, filters and pre-rendering.
// Test helpers outside `#[test]` functions panic on bad fixtures by design (clippy.toml only
// relaxes these lints inside test functions).
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use scorecraft_pdf::{Document, PdfError};
use scorecraft_render::{Filter, ScoreView, TILE_SIDE, ZoomBucket, neighbours};
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

fn view(pages: usize) -> ScoreView {
    let bytes = PdfBuilder::new().pages(pages, PageSpec::default()).build();
    ScoreView::new(
        Document::open(Arc::new(bytes)).unwrap(),
        scorecraft_render::DEFAULT_BUDGET,
    )
}

#[test]
fn a_second_request_is_a_cache_hit() {
    let mut v = view(3);
    let a = v.page(0, 1.0).unwrap();
    let b = v.page(0, 1.003).unwrap();
    assert!(Arc::ptr_eq(&a, &b), "nearby scales should share a bucket");
    let s = v.cache_stats();
    assert_eq!((s.hits, s.misses), (1, 1));
    assert_eq!(v.cached_bytes(), a.byte_len());
}

#[test]
fn renders_at_the_bucket_scale() {
    let mut v = view(1);
    let bmp = v.page(0, 1.5).unwrap();
    let s = ZoomBucket::of(1.5).unwrap().scale();
    assert_eq!(bmp.width, scorecraft_pdf::device_pixels(612.0, s));
}

#[test]
fn tiles_cover_the_page() {
    let mut v = view(1);
    let (cols, rows) = v.tile_grid(0, 2.0).unwrap();
    let whole = v.page(0, 2.0).unwrap();
    assert_eq!(
        (cols, rows),
        (
            whole.width.div_ceil(TILE_SIDE),
            whole.height.div_ceil(TILE_SIDE)
        )
    );
    let last = v.tile(0, 2.0, cols - 1, rows - 1).unwrap();
    assert_eq!(last.width, whole.width - (cols - 1) * TILE_SIDE);
    assert_eq!(last.height, whole.height - (rows - 1) * TILE_SIDE);
    let first = v.tile(0, 2.0, 0, 0).unwrap();
    assert_eq!((first.width, first.height), (TILE_SIDE, TILE_SIDE));
    assert_eq!(first.pixel(10, 10), whole.pixel(10, 10));
    assert_eq!(v.tile(0, 2.0, cols, 0).unwrap_err(), PdfError::BadTile);
    assert_eq!(v.tile(0, 2.0, u32::MAX, 0).unwrap_err(), PdfError::BadTile);
}

#[test]
fn filters_are_cached_separately() {
    let mut v = view(1);
    let normal = v.page(0, 0.5).unwrap();
    v.set_filter(Filter::Dark);
    let dark = v.page(0, 0.5).unwrap();
    assert_eq!(normal.pixel(1, 1), Some([255, 255, 255, 255]));
    assert_eq!(dark.pixel(1, 1), Some([0x1c, 0x1c, 0x1c, 255]));
    v.set_filter(Filter::Normal);
    assert!(Arc::ptr_eq(&normal, &v.page(0, 0.5).unwrap()));
}

#[test]
fn bad_requests_are_errors() {
    let mut v = view(2);
    assert_eq!(v.page(2, 1.0).unwrap_err().code(), "page_out_of_range");
    assert_eq!(v.page(0, f32::NAN).unwrap_err(), PdfError::BadScale);
    assert_eq!(v.page(0, 0.0).unwrap_err(), PdfError::BadScale);
    v.prefetch(&[0, 7, usize::MAX], -1.0);
    assert_eq!(v.pending(), 0);
}

#[test]
fn neighbours_are_current_next_previous() {
    assert_eq!(neighbours(0, 3), vec![0, 1]);
    assert_eq!(neighbours(1, 3), vec![1, 2, 0]);
    assert_eq!(neighbours(2, 3), vec![2, 1]);
    assert_eq!(neighbours(5, 3), Vec::<usize>::new());
    assert_eq!(neighbours(usize::MAX, usize::MAX), vec![usize::MAX - 1]);
}

#[test]
fn prefetch_on_the_callers_thread() {
    let mut v = view(4);
    v.prefetch(&neighbours(1, 4), 0.5);
    assert_eq!(v.pending(), 3);
    assert_eq!(v.run_prefetch(10), 3);
    for p in [0, 1, 2] {
        assert!(v.is_cached(p, 0.5), "page {p}");
    }
    assert!(!v.is_cached(3, 0.5));
    // A new request replaces the queue and skips what is cached.
    v.prefetch(&[3, 2], 0.5);
    v.prefetch(&[2, 1], 0.5);
    assert_eq!(v.pending(), 0);
}

#[test]
fn a_huge_timeout_does_not_overflow() {
    let v = view(1);
    assert!(v.wait_for_prefetch(Duration::MAX));
}

#[test]
fn background_worker_makes_turns_cache_hits() {
    let mut v = view(6);
    v.start_worker().unwrap();
    v.start_worker().unwrap();
    v.page(2, 0.5).unwrap();
    v.prefetch(&neighbours(2, 6), 0.5);
    assert!(v.wait_for_prefetch(Duration::from_secs(30)));
    let before = v.cache_stats();
    for p in [3, 1] {
        v.page(p, 0.5).unwrap();
    }
    let after = v.cache_stats();
    assert_eq!(after.hits - before.hits, 2, "turns should be cache hits");
    assert_eq!(after.misses, before.misses);
    // The worker follows a filter change.
    v.set_filter(Filter::Sepia);
    v.prefetch(&[4], 0.5);
    assert!(v.wait_for_prefetch(Duration::from_secs(30)));
    assert!(v.is_cached(4, 0.5));
}

#[test]
fn a_small_budget_evicts_old_pages() {
    let mut v = view(4);
    let one = v.page(0, 0.25).unwrap().byte_len();
    v.set_budget(one * 2);
    for p in 0..4 {
        v.page(p, 0.25).unwrap();
    }
    assert!(v.cached_bytes() <= one * 2);
    assert!(!v.is_cached(0, 0.25) && v.is_cached(3, 0.25));
}

#[test]
fn dropping_a_view_with_queued_work_stops_the_worker() {
    let mut v = view(20);
    v.start_worker().unwrap();
    v.prefetch(&(0..20).collect::<Vec<_>>(), 1.0);
    drop(v);
}

fn red_line() -> scorecraft_render::PageInk {
    use scorecraft_doc::{Color, InkPoint, Stroke, Tool};
    scorecraft_render::PageInk {
        key: 7,
        marks: Arc::new(vec![scorecraft_doc::AnnotationKind::Stroke(Stroke {
            tool: Tool::Pen,
            color: Color::RED,
            width: 10.0,
            nib: Default::default(),
            points: vec![InkPoint::new(100.0, 396.0), InkPoint::new(500.0, 396.0)],
        })]),
        curve: scorecraft_ink::PressureCurve::default(),
    }
}

#[test]
fn ink_is_drawn_in_the_background_before_a_turn_needs_it() {
    let mut v = view(3);
    v.start_worker().unwrap();
    v.set_ink(1, Some(red_line()));
    assert_eq!(v.ink_key(1), Some(7));
    v.prefetch(&[1], 1.0);
    assert!(v.wait_for_prefetch(Duration::from_secs(30)));
    assert!(v.is_inked_cached(1, 1.0), "the worker drew the ink");
    let inked = v.inked_page(1, 1.0).unwrap();
    let plain = v.page(1, 1.0).unwrap();
    assert!(!Arc::ptr_eq(&inked, &plain));
    let s = ZoomBucket::of(1.0).unwrap().scale();
    let y = ((792.0 - 396.0) * s) as u32;
    let px = inked.pixel((300.0 * s) as u32, y).unwrap();
    assert!(px[0] > 200 && px[1] < 60, "{px:?}");
    assert_ne!(plain.pixel((300.0 * s) as u32, y), Some(px));
    // A page without ink is the plain page; clearing ink returns to it.
    assert!(Arc::ptr_eq(
        &v.inked_page(0, 1.0).unwrap(),
        &v.page(0, 1.0).unwrap()
    ));
    v.set_ink(1, None);
    assert!(Arc::ptr_eq(&v.inked_page(1, 1.0).unwrap(), &plain));
}

#[test]
fn ink_without_a_worker_is_drawn_by_run_prefetch_or_on_demand() {
    let mut v = view(2);
    v.set_ink(0, Some(red_line()));
    assert!(!v.is_inked_cached(0, 1.0));
    v.prefetch(&[0], 1.0);
    assert_eq!(v.run_prefetch(4), 1);
    assert!(v.is_inked_cached(0, 1.0));
    v.set_ink(1, Some(red_line()));
    let _ = v.page(1, 1.0).unwrap();
    assert!(!v.is_inked_cached(1, 1.0), "cached plain, not yet inked");
    let _ = v.inked_page(1, 1.0).unwrap();
    assert!(v.is_inked_cached(1, 1.0));
}
