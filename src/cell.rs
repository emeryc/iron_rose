use fasthash::metro::hash64;
use serde::{Deserialize, Serialize};
use std::ops::{Add, Deref, Sub, SubAssign};

/// Which side of the IBF is this from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Was on the "Left" side and missing in the "Right" side
    Left(u128),
    /// Was on the "Right" side and missing in the "Left" side
    Right(u128),
}

impl Deref for Side {
    type Target = u128;

    fn deref(&self) -> &Self::Target {
        match self {
            Side::Left(l) => l,
            Side::Right(r) => r,
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, Default)]
pub(crate) struct Cell {
    // Are my sums the right size?
    id_sum: u128,
    hash_sum: u64,
    count: i32,
}
impl Cell {
    pub(crate) fn encode(&mut self, element: u128) {
        self.id_sum ^= element;
        self.hash_sum ^= hash64(element.to_be_bytes());
        self.count += 1;
    }

    pub(crate) fn is_pure(&self) -> bool {
        (self.count == 1 || self.count == -1) && self.hash_sum == hash64(self.id_sum.to_be_bytes())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0 && self.hash_sum == 0 && self.id_sum == 0
    }

    pub(crate) fn decode(&self) -> Result<Side, String> {
        if !self.is_pure() {
            return Err("Impure bucket".to_string());
        }
        Ok(if self.count == 1 {
            Side::Left(self.id_sum)
        } else {
            Side::Right(self.id_sum)
        })
    }
}

impl Add for Cell {
    type Output = Cell;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            id_sum: self.id_sum ^ rhs.id_sum,
            hash_sum: self.hash_sum ^ rhs.hash_sum,
            count: self.count + rhs.count,
        }
    }
}
impl SubAssign for Cell {
    fn sub_assign(&mut self, rhs: Self) {
        self.id_sum ^= rhs.id_sum;
        self.hash_sum ^= rhs.hash_sum;
        self.count -= rhs.count;
    }
}

impl Sub for Cell {
    type Output = Cell;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            id_sum: self.id_sum ^ rhs.id_sum,
            hash_sum: self.hash_sum ^ rhs.hash_sum,
            count: self.count - rhs.count,
        }
    }
}

impl Sub for &Cell {
    type Output = Cell;

    fn sub(self, rhs: Self) -> Self::Output {
        Cell {
            id_sum: self.id_sum ^ rhs.id_sum,
            hash_sum: self.hash_sum ^ rhs.hash_sum,
            count: self.count - rhs.count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut bucket: Cell = Default::default();
        bucket.encode(1);
        assert!(bucket.is_pure());
        assert_eq!(bucket.decode(), Ok(Side::Left(1)));
    }

    #[test]
    fn subtract() {
        let (mut b1, mut b2) = (Cell::default(), Cell::default());
        b1.encode(2);
        b1.encode(2);
        b2.encode(1);
        assert_eq!((b1 - b2).decode(), Ok(Side::Left(1)));
        assert_eq!((b2 - b1).decode(), Ok(Side::Right(1)));
    }

    #[test]
    fn impure() {
        let mut b1 = Cell::default();
        b1.encode(1);
        b1.encode(2);
        assert!(!b1.is_pure())
    }

    #[test]
    fn impure_disjoint() {
        let (mut b1, mut b2) = (Cell::default(), Cell::default());
        b1.encode(1);
        b1.encode(2);
        b2.encode(3);
        assert!(!(b1 - b2).is_pure());
    }
}
