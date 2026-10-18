use crate::cell::{Cell, Element};
use crate::error::{DecodeError, ShapeMismatch};
use crate::{hash, Side};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::ops::Sub;

/// Default number of cells each element is hashed into, as recommended by the paper.
pub const DEFAULT_HASH_COUNT: usize = 3;

/// Core Invertible Bloom Filter Data Structure. This allows us to store and differentially retrieve
/// a set of elements, provided that the IBF has enough cells for the number of differences.
/// This is a raw building block, and is useful for passing around IDs.
///
/// The cells are split into `hash_count` equally sized partitions, and each element is
/// stored in exactly one cell of each partition. This guarantees each element lands in
/// `hash_count` distinct cells.
/// ```rust
/// use iron_rose::{IBF, Side};
///
/// let mut left = IBF::new(20);
/// let mut right = IBF::new(20);
/// left.encode(10u128); left.encode(20); left.encode(30);
/// right.encode(10); right.encode(33); right.encode(42);
/// let diff = (left - right).expect("We are using two same sized IBFs");
/// let set = diff.decode().expect("We should be able to fully retrieve the data");
/// assert!(set.contains(&Side::Left(20)));
/// assert!(set.contains(&Side::Right(42)));
/// assert_eq!(set.len(), 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawIbf<T>")]
pub struct IBF<T> {
    cells: Box<[Cell<T>]>,
    hash_count: usize,
}

/// Unvalidated wire format, checked by `TryFrom` before becoming an [IBF].
#[derive(Deserialize)]
struct RawIbf<T> {
    cells: Box<[Cell<T>]>,
    hash_count: usize,
}

impl<T> TryFrom<RawIbf<T>> for IBF<T> {
    type Error = &'static str;

    fn try_from(raw: RawIbf<T>) -> Result<Self, Self::Error> {
        if raw.hash_count == 0 {
            return Err("IBF hash_count must be at least 1");
        }
        let per_partition = raw.cells.len() / raw.hash_count;
        if per_partition == 0 || per_partition * raw.hash_count != raw.cells.len() {
            return Err("IBF cell count must be a non-zero multiple of hash_count");
        }
        Ok(IBF {
            cells: raw.cells,
            hash_count: raw.hash_count,
        })
    }
}

impl<T: Element> IBF<T> {
    /// New IBF with at least `size` cells and a default hash_count of 3 (as per the paper).
    ///
    /// For an IBF to decode reliably it needs roughly 1.5x-2x as many cells as the number of
    /// differences it will hold. [StrataEstimator::recommended_ibf_size](crate::StrataEstimator::recommended_ibf_size)
    /// can help pick a size.
    pub fn new(size: usize) -> Self {
        Self::new_with_hash_count(size, DEFAULT_HASH_COUNT)
    }

    /// New IBF with at least `size` cells, where each element is stored in `hash_count` cells.
    ///
    /// `size` is rounded up to the next non-zero multiple of `hash_count`.
    ///
    /// # Panics
    /// If `hash_count` is 0.
    pub fn new_with_hash_count(size: usize, hash_count: usize) -> Self {
        assert!(hash_count > 0, "hash_count must be at least 1");
        let per_partition = size.div_ceil(hash_count).max(1);
        Self {
            cells: vec![Cell::default(); per_partition * hash_count].into_boxed_slice(),
            hash_count,
        }
    }

    /// Total number of cells in this IBF.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Number of cells each element is stored in.
    pub fn hash_count(&self) -> usize {
        self.hash_count
    }

    /// True if nothing has been encoded (or, after a subtraction, the two sides were equal).
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(Cell::is_empty)
    }

    /// Encodes an element into hash_count # of cells for future retrieval
    pub fn encode(&mut self, element: T) {
        let checksum = hash::checksum(&element);
        for idx in self.indices(&element) {
            self.cells[idx].encode(element.clone(), checksum);
        }
    }

    /// Decodes an IBF into a [HashSet] of [Sides](Side). Each side tells you which of the
    /// original IBFs the element came from (after a subtraction). Returns a [DecodeError],
    /// which includes everything recovered so far, if the IBF doesn't hold enough
    /// information to be fully decoded.
    pub fn decode(mut self) -> Result<HashSet<Side<T>>, DecodeError<T>> {
        let mut set = HashSet::new();
        let mut pure: Vec<usize> = (0..self.cells.len())
            .filter(|&i| self.cells[i].is_pure())
            .collect();

        // Each peel empties a cell, which (for an honestly built IBF) never gets touched again,
        // so more peels than cells means the IBF is corrupt or hostile. The cap guarantees we
        // terminate on untrusted input.
        let mut peels_left = self.cells.len();
        while let Some(idx) = pure.pop() {
            // A cell can stop being pure after it was queued, if a neighbour was peeled first.
            let side = match self.cells[idx].decode() {
                Some(side) => side,
                None => continue,
            };
            let indices: Vec<usize> = self.indices(&side).collect();
            // A pure-looking cell outside its element's own cells can only come from a
            // corrupt/hostile IBF. Peeling it wouldn't empty it, so leave it undecoded.
            if !indices.contains(&idx) || peels_left == 0 {
                continue;
            }
            peels_left -= 1;
            let cell = self.cells[idx].clone();
            for i in indices {
                self.cells[i] -= &cell;
                if self.cells[i].is_pure() {
                    pure.push(i);
                }
            }
            set.insert(side);
        }

        let remaining_cells = self.cells.iter().filter(|c| !c.is_empty()).count();
        if remaining_cells == 0 {
            Ok(set)
        } else {
            Err(DecodeError {
                partial: set,
                remaining_cells,
            })
        }
    }

    /// The index of the cell `element` is stored in for each partition.
    fn indices(&self, element: &T) -> impl Iterator<Item = usize> {
        let base = hash::index_hash(element);
        let per_partition = (self.cells.len() / self.hash_count) as u64;
        (0..self.hash_count as u64).map(move |i| {
            let offset = hash::partition_hash(base, i) % per_partition;
            (i * per_partition + offset) as usize
        })
    }
}

impl<T: Element> Sub for &IBF<T> {
    type Output = Result<IBF<T>, ShapeMismatch>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.hash_count != rhs.hash_count || self.cells.len() != rhs.cells.len() {
            return Err(ShapeMismatch);
        }
        Ok(IBF {
            cells: self
                .cells
                .iter()
                .zip(rhs.cells.iter())
                .map(|(l, r)| l - r)
                .collect(),
            hash_count: self.hash_count,
        })
    }
}

impl<T: Element> Sub for IBF<T> {
    type Output = Result<IBF<T>, ShapeMismatch>;

    fn sub(self, rhs: Self) -> Self::Output {
        &self - &rhs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small deterministic PRNG so tests don't need a `rand` dependency.
    fn lcg(seed: &mut u128) -> u128 {
        *seed = seed
            .wrapping_mul(0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645)
            .wrapping_add(1);
        *seed
    }

    fn sym_diff_ibfs(
        common: usize,
        diff: usize,
        size: usize,
        seed: &mut u128,
    ) -> (IBF<u128>, HashSet<Side<u128>>) {
        let (mut l, mut r) = (IBF::new(size), IBF::new(size));
        let mut expected = HashSet::new();
        for _ in 0..common {
            let x = lcg(seed);
            l.encode(x);
            r.encode(x);
        }
        for i in 0..diff {
            let x = lcg(seed);
            if i % 2 == 0 {
                l.encode(x);
                expected.insert(Side::Left(x));
            } else {
                r.encode(x);
                expected.insert(Side::Right(x));
            }
        }
        ((l - r).unwrap(), expected)
    }

    #[test]
    fn single_cell_per_partition() {
        // Previously all hashes could land on the same cell, making this undecodable.
        let mut ibf = IBF::new(1);
        assert_eq!(ibf.cell_count(), 3);
        ibf.encode(7u128);
        assert_eq!(ibf.decode().unwrap(), [Side::Left(7)].into_iter().collect());
    }

    #[test]
    fn size_is_rounded_up() {
        assert_eq!(IBF::<u64>::new(0).cell_count(), 3);
        assert_eq!(IBF::<u64>::new(10).cell_count(), 12);
        assert_eq!(IBF::<u64>::new_with_hash_count(10, 4).cell_count(), 12);
        assert_eq!(IBF::<u64>::new_with_hash_count(10, 1).cell_count(), 10);
    }

    #[test]
    #[should_panic]
    fn zero_hash_count_panics() {
        IBF::<u64>::new_with_hash_count(10, 0);
    }

    #[test]
    fn empty_decodes_to_nothing() {
        let ibf = IBF::<u64>::new(30);
        assert!(ibf.is_empty());
        assert!(ibf.decode().unwrap().is_empty());
    }

    #[test]
    fn shape_mismatch() {
        let a = IBF::<u64>::new(30);
        assert_eq!(&a - &IBF::new(60), Err(ShapeMismatch));
        assert_eq!(&a - &IBF::new_with_hash_count(30, 5), Err(ShapeMismatch));
    }

    #[test]
    fn decodes_reliably_with_headroom() {
        // 2d + 30 is what StrataEstimator::recommended_ibf_size suggests.
        let mut seed = 42;
        for &d in &[1usize, 10, 50, 100, 1000] {
            let mut ok = 0;
            for _ in 0..50 {
                let (diff, expected) = sym_diff_ibfs(2000, d, 2 * d + 30, &mut seed);
                if diff.decode().ok() == Some(expected) {
                    ok += 1;
                }
            }
            assert!(ok >= 48, "d={} only decoded {}/50", d, ok);
        }
    }

    #[test]
    fn large_ibfs_decode_at_1_5x() {
        let mut seed = 1;
        let (diff, expected) = sym_diff_ibfs(1000, 10_000, 15_000, &mut seed);
        assert_eq!(diff.decode().unwrap(), expected);
    }

    #[test]
    fn decode_failure_returns_partial() {
        let mut seed = 7;
        let (diff, expected) = sym_diff_ibfs(100, 200, 30, &mut seed);
        let err = diff.decode().unwrap_err();
        assert!(err.remaining_cells > 0);
        assert!(err.partial.is_subset(&expected));
    }

    #[test]
    fn serde_round_trip() {
        let mut ibf = IBF::new(30);
        ibf.encode(5u64);
        let json = serde_json::to_string(&ibf).unwrap();
        let back: IBF<u64> = serde_json::from_str(&json).unwrap();
        assert_eq!(ibf, back);
    }

    #[test]
    fn serde_rejects_bad_shapes() {
        let cell = r#"{"id_sum":0,"hash_sum":0,"count":0}"#;
        let bad = [
            r#"{"cells":[],"hash_count":3}"#.to_string(),
            format!(r#"{{"cells":[{c},{c}],"hash_count":3}}"#, c = cell),
            format!(r#"{{"cells":[{c},{c},{c}],"hash_count":0}}"#, c = cell),
        ];
        for json in bad.iter() {
            assert!(serde_json::from_str::<IBF<u64>>(json).is_err(), "{}", json);
        }
        let good = format!(r#"{{"cells":[{c},{c},{c}],"hash_count":3}}"#, c = cell);
        assert!(serde_json::from_str::<IBF<u64>>(&good).is_ok());
    }
}
