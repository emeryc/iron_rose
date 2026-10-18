use crate::Side;
use std::collections::HashSet;
use std::fmt;

/// Two IBFs or strata estimators can't be combined because they're configured differently
/// (different number of cells, hash functions, or strata).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeMismatch;

impl fmt::Display for ShapeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IBFs / strata estimators are not configured the same")
    }
}

impl std::error::Error for ShapeMismatch {}

/// An [IBF](crate::IBF) couldn't be fully decoded, usually because it was too small for the
/// number of differences it holds.
#[derive(Debug, Clone)]
pub struct DecodeError<T> {
    /// Elements that were recovered before decoding got stuck. These are correct (with very high
    /// probability), but the set is incomplete.
    pub partial: HashSet<Side<T>>,
    /// Number of cells that were still non-empty when decoding got stuck.
    pub remaining_cells: usize,
}

impl<T> fmt::Display for DecodeError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unable to fully decode IBF: recovered {} elements, {} cells still non-empty",
            self.partial.len(),
            self.remaining_cells
        )
    }
}

impl<T: fmt::Debug> std::error::Error for DecodeError<T> {}
