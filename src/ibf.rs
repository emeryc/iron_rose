use std::{collections::HashSet, ops::Sub};

use crate::{cell::Cell, Side};
use fasthash::murmur2::hash64;
use serde::{Deserialize, Serialize};

/// Core Invertable Bloom Filter Data Structure. This allows us to store and differentially retreive
/// a set of u128s, provided that the two IBFs have enough information in them. This is a
/// raw building block, and is useful for passing around IDs.
/// ```rust
/// use iron_rose::{IBF, Side};
///
/// let mut left = IBF::new(20);
/// let mut right = IBF::new(20);
/// left.encode(10); left.encode(20); left.encode(30);
/// right.encode(10); right.encode(33); right.encode(42);
/// let mut diff = (left - right).expect("We are using two same sized IBFs");
/// let set = diff.decode().expect("We should be able to fully retreive the data");
/// assert!(set.contains(&Side::Left(20)));
/// assert!(set.contains(&Side::Right(42)));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IBF {
    cells: Box<[Cell]>,
    hash_count: usize,
    size: usize,
}

impl IBF {
    /// New IBF, limited to having size number of buckets, and a default hash_count of 3 (as per the paper)
    pub fn new(size: usize) -> Self {
        Self::new_with_hash_count(size, 3)
    }

    /// New IBF, limited to having size number of buckets, and a settable hash_count
    pub fn new_with_hash_count(size: usize, hash_count: usize) -> Self {
        let buckets = vec![Cell::default(); size].into_boxed_slice();
        Self {
            cells: buckets,
            hash_count,
            size,
        }
    }

    /// Encodes an element into hash_count # of buckets for future retreival
    pub fn encode(&mut self, element: u128) {
        for i in 0..self.hash_count {
            self.get_ith_cell(i, element).encode(element)
        }
    }

    /// Allows you to decode an IBF into a [HashSet](HashSet) of [Sides](Side). Each side tells
    /// You from which original IBF the data came from (After a subtraction). Returns an Err
    /// In the case that we don't have enough information to fully decode the IBF.
    pub fn decode(mut self) -> Result<HashSet<Side>, String> {
        let mut set = HashSet::new();
        loop {
            if let Some(next_pure) = self.cells.iter().find(|cell| cell.is_pure()) {
                let next_pure = *next_pure;
                let element = next_pure.decode().expect("Only grabbing pure elements");
                set.insert(element);
                self.remove(next_pure);
            } else {
                if self.cells.iter().all(|cell| cell.is_empty()) {
                    return Ok(set);
                } else {
                    let not_empty = self
                        .cells
                        .iter()
                        .filter(|cell| !cell.is_empty())
                        .collect::<Vec<_>>();
                    return Err(format!("Unable to fully decode: {:#?}", not_empty));
                }
            }
        }
    }

    fn remove(&mut self, cell: Cell) {
        let element = *cell.decode().expect("Only removing pure cells");
        for i in 0..self.hash_count {
            *self.get_ith_cell(i, element) -= cell;
        }
    }

    fn get_ith_cell(&mut self, i: usize, element: u128) -> &mut Cell {
        let cell_idx =
            (hash64((element.wrapping_add(i as u128)).to_be_bytes()) % (self.size as u64)) as usize;
        &mut self.cells[cell_idx]
    }
}

impl Sub for IBF {
    type Output = Result<IBF, String>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.hash_count != rhs.hash_count || self.size != rhs.size {
            return Err("IBFs are not configured the same".to_string());
        }
        Ok(Self {
            cells: self
                .cells
                .iter()
                .zip(rhs.cells.iter())
                .map(|(l, r)| l - r)
                .collect(),
            ..self
        })
    }
}

impl Sub for &IBF {
    type Output = Result<IBF, String>;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.hash_count != rhs.hash_count || self.size != rhs.size {
            return Err("IBFs are not configured the same".to_string());
        }
        Ok(IBF {
            cells: self
                .cells
                .iter()
                .zip(rhs.cells.iter())
                .map(|(l, r)| l - r)
                .collect(),
            hash_count: self.hash_count,
            size: self.size,
        })
    }
}
