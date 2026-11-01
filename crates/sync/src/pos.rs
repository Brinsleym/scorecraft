//! Positions for list order: a sequence CRDT by fractional index. Every entry of a list carries
//! a position, a list of digits compared in dictionary order; an entry moved or inserted gets a
//! new position between its neighbours', so concurrent inserts never renumber anything. Two
//! entries given the same position concurrently order by their ids.

use serde::{Deserialize, Serialize};

/// A place in a list's order.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pos(pub Vec<u32>);

/// One past the largest digit.
const BASE: u64 = 1 << 32;

impl Pos {
    /// The position for the `i`th entry of a list that has no positions yet.
    pub fn nth(i: usize) -> Pos {
        let d = u64::try_from(i).unwrap_or(u64::MAX).saturating_add(1) << 10;
        Pos(vec![u32::try_from(d).unwrap_or(u32::MAX)])
    }

    /// [`Pos::between`] with `salt` (not 0) added as a last digit: still between, and unlikely
    /// to equal a position another device chose for the same gap.
    pub fn between_for(lo: Option<&Pos>, hi: Option<&Pos>, salt: u32) -> Pos {
        let mut p = Pos::between(lo, hi);
        p.0.push(salt.max(1));
        p
    }

    /// A position after `lo` (or at the start) and before `hi` (or at the end). When `hi` is not
    /// after `lo` (two positions given concurrently), the result is just after `lo`.
    pub fn between(lo: Option<&Pos>, hi: Option<&Pos>) -> Pos {
        let lo_d: &[u32] = lo.map_or(&[], |p| p.0.as_slice());
        let hi = hi.filter(|h| lo.is_none_or(|l| *h > l));
        let mut out = Vec::new();
        // While `bounded`, the digits so far equal `hi`'s, so `hi` still limits the next one.
        let mut bounded = hi.is_some();
        let hi_d: &[u32] = hi.map_or(&[], |p| p.0.as_slice());
        for i in 0.. {
            let l = u64::from(lo_d.get(i).copied().unwrap_or(0));
            let h = if bounded {
                hi_d.get(i).map_or(BASE, |&d| u64::from(d))
            } else {
                BASE
            };
            if h > l + 1 {
                // The middle digit is above `l`, so the result is after `lo`, and below `h`.
                out.push(u32::try_from(l + (h - l) / 2).unwrap_or(u32::MAX));
                return Pos(out);
            }
            out.push(u32::try_from(l).unwrap_or(u32::MAX));
            if h > l {
                bounded = false;
            }
            if i > lo_d.len() + hi_d.len() + 2 {
                break;
            }
        }
        // Not reachable for well-formed positions (last digits are never 0); stay after `lo`.
        let mut p = lo_d.to_vec();
        p.push(1 << 31);
        Pos(p)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn between_is_strictly_between() {
        let a = Pos(vec![5]);
        let b = Pos(vec![6]);
        let m = Pos::between(Some(&a), Some(&b));
        assert!(a < m && m < b, "{m:?}");
        let s = Pos::between(None, Some(&Pos::nth(0)));
        assert!(s < Pos::nth(0));
        let e = Pos::between(Some(&Pos::nth(3)), None);
        assert!(e > Pos::nth(3));
        // A backwards pair (concurrent) still lands after the lower bound.
        let x = Pos::between(Some(&b), Some(&a));
        assert!(x > b);
    }

    proptest! {
        #[test]
        fn repeated_inserts_stay_ordered(choices in proptest::collection::vec(any::<u16>(), 1..200)) {
            let mut list: Vec<Pos> = Vec::new();
            for c in choices {
                let i = usize::from(c) % (list.len() + 1);
                let lo = i.checked_sub(1).and_then(|j| list.get(j));
                let p = Pos::between(lo, list.get(i));
                list.insert(i, p);
            }
            for w in list.windows(2) {
                prop_assert!(w.first().unwrap() < w.get(1).unwrap());
            }
            for p in &list {
                prop_assert!(p.0.last().is_some_and(|&d| d != 0));
            }
        }
    }
}
