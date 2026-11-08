//! The reader's position in a score and how it turns (spec: `docs/spec/page-turning.md`).
//!
//! A score is read as a sequence of [`View`]s. The layout groups pages into *spreads* (one page,
//! or two facing pages); the turn mode decides which views come between two spreads. Views are
//! computed from their index, never stored, so a 100,000-page file costs nothing.

/// How pages are grouped on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One page at a time (READ-2).
    Single,
    /// Two facing pages (READ-3): 1–2, 3–4, … (The owner removed "first page alone" on
    /// 2026-11-01.)
    TwoPage,
}

impl Layout {
    pub fn id(self) -> &'static str {
        match self {
            Layout::Single => "single",
            Layout::TwoPage => "twoPage",
        }
    }

    /// Number of spreads in a score of `count` pages.
    fn spread_count(self, count: usize) -> usize {
        match self {
            _ if count == 0 => 0,
            Layout::Single => count,
            Layout::TwoPage => count.div_ceil(2),
        }
    }

    /// The pages of spread `i`, left to right.
    fn spread(self, i: usize, count: usize) -> Vec<usize> {
        let (a, b) = match self {
            Layout::Single => (Some(i), None),
            Layout::TwoPage => {
                let a = i.saturating_mul(2);
                (Some(a), a.checked_add(1))
            }
        };
        [a, b]
            .into_iter()
            .flatten()
            .filter(|&p| p < count)
            .collect()
    }

    /// The spread that holds `page`.
    fn spread_of(self, page: usize) -> usize {
        match self {
            Layout::Single => page,
            Layout::TwoPage => page / 2,
        }
    }
}

/// What a turn does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnMode {
    /// The whole screen moves to the next spread.
    Full,
    /// Half-page turns (READ-5): the first turn replaces the top part of the screen with the top
    /// of the next spread while the bottom part, which the player is reading, stays put; the
    /// second turn completes it.
    HalfPage,
    /// Alternating turns (READ-6), two-page layout only: each turn replaces the page the player
    /// has just finished and leaves the one they are reading where it is (1–2, 3–2, 3–4, 5–4, …).
    /// In the single-page layout it turns like [`TurnMode::Full`].
    Alternating,
}

impl TurnMode {
    pub fn id(self) -> &'static str {
        match self {
            TurnMode::Full => "full",
            TurnMode::HalfPage => "halfPage",
            TurnMode::Alternating => "alternating",
        }
    }
}

/// One screenful. For a full view `top == bottom`; a half-page view shows the top part of `top`
/// above the bottom part of `bottom`. Pages are 0-based and listed left to right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub top: Vec<usize>,
    pub bottom: Vec<usize>,
}

impl View {
    fn full(pages: Vec<usize>) -> View {
        View {
            top: pages.clone(),
            bottom: pages,
        }
    }

    /// True while a half-page turn is in progress.
    pub fn is_half(&self) -> bool {
        self.top != self.bottom
    }

    /// Every page on screen, in reading order, each once.
    pub fn pages(&self) -> Vec<usize> {
        let mut v = self.bottom.clone();
        for p in &self.top {
            if !v.contains(p) {
                v.push(*p);
            }
        }
        v
    }

    /// The page the player is reading: the lowest page of a full view, the lowest page of the
    /// bottom part during a half-page turn.
    pub fn anchor(&self) -> usize {
        self.bottom.iter().copied().min().unwrap_or(0)
    }
}

/// Every view of a score in turn order. For tests and tools; the [`Reader`] computes views on
/// demand.
pub fn views(count: usize, layout: Layout, mode: TurnMode) -> Vec<View> {
    let seq = Sequence {
        count,
        layout,
        mode,
    };
    (0..seq.len()).filter_map(|i| seq.view(i)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sequence {
    count: usize,
    layout: Layout,
    mode: TurnMode,
}

impl Sequence {
    /// Alternation applies to facing pages only.
    fn alternating(&self) -> bool {
        self.mode == TurnMode::Alternating && self.layout == Layout::TwoPage
    }

    /// With alternating turns: the number of views before alternation starts and the page
    /// alternation starts from (both 0 now every spread is a pair).
    fn alternation_start(&self) -> (usize, usize) {
        (0, 0)
    }

    fn len(&self) -> usize {
        let spreads = self.layout.spread_count(self.count);
        if spreads == 0 {
            return 0;
        }
        if self.alternating() {
            let (lead, start) = self.alternation_start();
            // Pages start..count alternate: one view for one page, m - 1 views for m >= 2.
            let m = self.count.saturating_sub(start);
            return lead + m.saturating_sub(1).max(usize::from(m > 0));
        }
        match self.mode {
            TurnMode::HalfPage => spreads.saturating_mul(2) - 1,
            _ => spreads,
        }
    }

    fn view(&self, i: usize) -> Option<View> {
        if i >= self.len() {
            return None;
        }
        if self.alternating() {
            let (lead, start) = self.alternation_start();
            if i < lead {
                return Some(View::full(vec![0]));
            }
            let j = i - lead;
            let m = self.count.saturating_sub(start);
            if m == 1 {
                return Some(View::full(vec![start]));
            }
            // View j shows page k = start + 1 + j beside page k - 1, k on the side the page
            // before last occupied.
            let k = start.saturating_add(1).saturating_add(j);
            let pages = if (k - start) % 2 == 1 {
                vec![k - 1, k]
            } else {
                vec![k, k - 1]
            };
            return Some(View::full(pages));
        }
        match self.mode {
            TurnMode::HalfPage => {
                let s = i / 2;
                let here = self.layout.spread(s, self.count);
                if i.is_multiple_of(2) {
                    Some(View::full(here))
                } else {
                    Some(View {
                        top: self.layout.spread(s + 1, self.count),
                        bottom: here,
                    })
                }
            }
            _ => Some(View::full(self.layout.spread(i, self.count))),
        }
    }

    /// The first full view that shows `page` (clamped to the score).
    fn index_of(&self, page: usize) -> usize {
        let page = page.min(self.count.saturating_sub(1));
        if self.alternating() {
            let (lead, start) = self.alternation_start();
            if page < start {
                return 0;
            }
            // Page k first appears in view k - start - 1 (page `start` in the first one).
            let j = page.saturating_sub(start).saturating_sub(1);
            return (lead + j).min(self.len().saturating_sub(1));
        }
        let s = self.layout.spread_of(page);
        let i = match self.mode {
            TurnMode::HalfPage => s.saturating_mul(2),
            _ => s,
        };
        i.min(self.len().saturating_sub(1))
    }
}

/// Lowest and highest accepted half-page split (fraction of the page above the line).
pub const SPLIT_RANGE: (f64, f64) = (0.2, 0.8);

/// Where the reader is in a score and how it turns.
#[derive(Debug, Clone, PartialEq)]
pub struct Reader {
    seq: Sequence,
    index: usize,
    split: f64,
}

impl Reader {
    /// A reader at the first page of a score of `count` pages: single layout, full turns.
    pub fn new(count: usize) -> Reader {
        Reader {
            seq: Sequence {
                count,
                layout: Layout::Single,
                mode: TurnMode::Full,
            },
            index: 0,
            split: 0.5,
        }
    }

    pub fn page_count(&self) -> usize {
        self.seq.count
    }

    pub fn layout(&self) -> Layout {
        self.seq.layout
    }

    pub fn mode(&self) -> TurnMode {
        self.seq.mode
    }

    /// Where a half-page turn cuts the page: the fraction of the page height shown from the next
    /// spread.
    pub fn split(&self) -> f64 {
        self.split
    }

    /// Sets the half-page split, clamped to [`SPLIT_RANGE`]; anything not a number keeps it.
    pub fn set_split(&mut self, split: f64) {
        if split.is_finite() {
            self.split = split.clamp(SPLIT_RANGE.0, SPLIT_RANGE.1);
        }
    }

    /// Changes the layout, staying on the page being read.
    pub fn set_layout(&mut self, layout: Layout) {
        self.reseat(|s| s.layout = layout);
    }

    /// Changes the turn mode, staying on the page being read.
    pub fn set_mode(&mut self, mode: TurnMode) {
        self.reseat(|s| s.mode = mode);
    }

    fn reseat(&mut self, change: impl FnOnce(&mut Sequence)) {
        let anchor = self.view().map_or(0, |v| v.anchor());
        change(&mut self.seq);
        self.index = self.seq.index_of(anchor);
    }

    /// Number of views in turn order.
    pub fn len(&self) -> usize {
        self.seq.len()
    }

    /// True for a score with no pages.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The 0-based position in turn order.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The view on screen; `None` only for a score with no pages.
    pub fn view(&self) -> Option<View> {
        self.seq.view(self.index)
    }

    /// The views one turn back and one turn forward, for pre-rendering.
    pub fn neighbours(&self) -> (Option<View>, Option<View>) {
        let prev = self.index.checked_sub(1).and_then(|i| self.seq.view(i));
        let next = self.index.checked_add(1).and_then(|i| self.seq.view(i));
        (prev, next)
    }

    /// Turns forward; false (and nothing changes) on the last view.
    pub fn turn_next(&mut self) -> bool {
        if self.index + 1 < self.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    /// Turns back; false on the first view. Turning back during a half-page turn undoes it.
    pub fn turn_previous(&mut self) -> bool {
        if self.index > 0 {
            self.index -= 1;
            true
        } else {
            false
        }
    }

    /// Goes to the first full view showing `page` (0-based, clamped to the score).
    pub fn goto(&mut self, page: usize) {
        self.index = self.seq.index_of(page);
    }

    /// The page being read (see [`View::anchor`]).
    pub fn page(&self) -> usize {
        self.view().map_or(0, |v| v.anchor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tops(count: usize, layout: Layout, mode: TurnMode) -> Vec<(Vec<usize>, Vec<usize>)> {
        views(count, layout, mode)
            .into_iter()
            .map(|v| (v.top, v.bottom))
            .collect()
    }

    fn full(pages: &[&[usize]]) -> Vec<(Vec<usize>, Vec<usize>)> {
        pages.iter().map(|p| (p.to_vec(), p.to_vec())).collect()
    }

    const TWO: Layout = Layout::TwoPage;

    #[test]
    fn single_and_two_page_spreads() {
        assert_eq!(
            tops(3, Layout::Single, TurnMode::Full),
            full(&[&[0], &[1], &[2]])
        );
        assert_eq!(
            tops(5, TWO, TurnMode::Full),
            full(&[&[0, 1], &[2, 3], &[4]])
        );
    }

    #[test]
    fn half_page_turns_interleave_half_views() {
        let v = tops(3, Layout::Single, TurnMode::HalfPage);
        assert_eq!(
            v,
            vec![
                (vec![0], vec![0]),
                (vec![1], vec![0]),
                (vec![1], vec![1]),
                (vec![2], vec![1]),
                (vec![2], vec![2]),
            ]
        );
        let v = tops(3, TWO, TurnMode::HalfPage);
        assert_eq!(
            v,
            vec![
                (vec![0, 1], vec![0, 1]),
                (vec![2], vec![0, 1]),
                (vec![2], vec![2])
            ]
        );
    }

    #[test]
    fn alternating_replaces_the_finished_page() {
        assert_eq!(
            tops(5, TWO, TurnMode::Alternating),
            full(&[&[0, 1], &[2, 1], &[2, 3], &[4, 3]])
        );
        assert_eq!(tops(1, TWO, TurnMode::Alternating), full(&[&[0]]));
        assert_eq!(tops(2, TWO, TurnMode::Alternating), full(&[&[0, 1]]));
        // Single layout: alternating turns like full turns.
        assert_eq!(
            tops(2, Layout::Single, TurnMode::Alternating),
            full(&[&[0], &[1]])
        );
    }

    #[test]
    fn empty_scores_have_no_views() {
        for layout in [Layout::Single, TWO] {
            for mode in [TurnMode::Full, TurnMode::HalfPage, TurnMode::Alternating] {
                assert!(views(0, layout, mode).is_empty());
                let mut r = Reader::new(0);
                r.set_layout(layout);
                r.set_mode(mode);
                assert!(r.view().is_none() && !r.turn_next() && !r.turn_previous());
                r.goto(10);
                assert_eq!(r.page(), 0);
            }
        }
    }

    #[test]
    fn every_page_is_reachable_and_goto_lands_on_a_full_view() {
        for count in 1..12 {
            for layout in [Layout::Single, TWO] {
                for mode in [TurnMode::Full, TurnMode::HalfPage, TurnMode::Alternating] {
                    let all = views(count, layout, mode);
                    assert_eq!(all.len(), Reader::new(count).seq_len(layout, mode));
                    let mut seen: Vec<usize> = all.iter().flat_map(View::pages).collect();
                    seen.sort_unstable();
                    seen.dedup();
                    assert_eq!(
                        seen,
                        (0..count).collect::<Vec<_>>(),
                        "{count} {layout:?} {mode:?}"
                    );
                    let mut r = Reader::new(count);
                    r.set_layout(layout);
                    r.set_mode(mode);
                    for p in 0..count {
                        r.goto(p);
                        let v = r.view().unwrap();
                        assert!(!v.is_half() && v.pages().contains(&p), "{p} {v:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn turning_walks_the_sequence_and_stops_at_the_ends() {
        let mut r = Reader::new(3);
        r.set_mode(TurnMode::HalfPage);
        assert!(!r.turn_previous());
        let mut steps = 0;
        while r.turn_next() {
            steps += 1;
        }
        assert_eq!((steps, r.page()), (4, 2));
        assert!(r.turn_previous());
        assert!(r.view().unwrap().is_half());
        assert_eq!(r.page(), 1);
    }

    #[test]
    fn changing_layout_or_mode_keeps_the_page() {
        let mut r = Reader::new(10);
        r.goto(5);
        r.set_layout(TWO);
        assert_eq!(r.view().unwrap().top, vec![4, 5]);
        r.set_mode(TurnMode::HalfPage);
        r.turn_next();
        assert!(r.view().unwrap().is_half());
        r.set_mode(TurnMode::Full);
        assert_eq!(r.view().unwrap().top, vec![4, 5]);
        // The lowest page on screen is the one kept.
        r.set_layout(Layout::Single);
        assert_eq!(r.page(), 4);
    }

    #[test]
    fn split_is_clamped() {
        let mut r = Reader::new(2);
        r.set_split(0.9);
        assert_eq!(r.split(), 0.8);
        r.set_split(f64::NAN);
        assert_eq!(r.split(), 0.8);
        r.set_split(0.0);
        assert_eq!(r.split(), 0.2);
    }

    #[test]
    fn huge_scores_cost_nothing() {
        let mut r = Reader::new(usize::MAX / 4);
        r.set_layout(TWO);
        r.set_mode(TurnMode::HalfPage);
        r.goto(usize::MAX);
        assert!(!r.turn_next());
        assert!(r.view().is_some());
    }

    impl Reader {
        fn seq_len(&self, layout: Layout, mode: TurnMode) -> usize {
            Sequence {
                count: self.seq.count,
                layout,
                mode,
            }
            .len()
        }
    }
}
