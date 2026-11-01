//! The parity floor: the number of feature-map rows with a live command must never drop below
//! this. `cargo xtask parity` fails under it. Raise it when parity rises; never lower it.

/// Live feature-map rows (measured by `cargo xtask parity`).
pub const FLOOR: usize = 48;
