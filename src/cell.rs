use crate::hash;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{BitXor, BitXorAssign, Deref, Sub, SubAssign};

/// Anything that can be stored in an [IBF](crate::IBF).
///
/// This is implemented automatically for every type that meets the bounds, e.g. all of the
/// unsigned and signed integer types.
pub trait Element:
    Clone + Hash + BitXor<Output = Self> + BitXorAssign + Default + Eq + Debug
{
}

impl<T> Element for T where
    T: Clone + Hash + BitXor<Output = T> + BitXorAssign + Default + Eq + Debug
{
}

/// Which side of the IBF is this from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side<T> {
    /// Was on the "Left" side and missing in the "Right" side
    Left(T),
    /// Was on the "Right" side and missing in the "Left" side
    Right(T),
}

impl<T> Side<T> {
    /// Consumes the side, returning the inner element
    pub fn into_inner(self) -> T {
        match self {
            Side::Left(t) | Side::Right(t) => t,
        }
    }
}

impl<T> Deref for Side<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Side::Left(t) | Side::Right(t) => t,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
pub(crate) struct Cell<T> {
    id_sum: T,
    hash_sum: u64,
    count: i64,
}

impl<T: Element> Cell<T> {
    /// Adds `element` (whose checksum is `checksum`) to this cell.
    pub(crate) fn encode(&mut self, element: T, checksum: u64) {
        self.id_sum ^= element;
        self.hash_sum ^= checksum;
        self.count = self.count.wrapping_add(1);
    }

    pub(crate) fn is_pure(&self) -> bool {
        (self.count == 1 || self.count == -1) && self.hash_sum == hash::checksum(&self.id_sum)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0 && self.hash_sum == 0 && self.id_sum == T::default()
    }

    /// Returns which side this cell's single element came from, or None if the cell isn't pure.
    pub(crate) fn decode(&self) -> Option<Side<T>> {
        if !self.is_pure() {
            return None;
        }
        Some(if self.count == 1 {
            Side::Left(self.id_sum.clone())
        } else {
            Side::Right(self.id_sum.clone())
        })
    }
}

impl<T: Element> SubAssign<&Cell<T>> for Cell<T> {
    fn sub_assign(&mut self, rhs: &Self) {
        self.id_sum ^= rhs.id_sum.clone();
        self.hash_sum ^= rhs.hash_sum;
        // Counts only need to cancel out, so wrap rather than overflow on hostile input.
        self.count = self.count.wrapping_sub(rhs.count);
    }
}

impl<T: Element> Sub for &Cell<T> {
    type Output = Cell<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        let mut out = self.clone();
        out -= rhs;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(cell: &mut Cell<u128>, x: u128) {
        cell.encode(x, hash::checksum(&x));
    }

    #[test]
    fn round_trip() {
        let mut bucket = Cell::default();
        encode(&mut bucket, 1);
        assert!(bucket.is_pure());
        assert_eq!(bucket.decode(), Some(Side::Left(1)));
    }

    #[test]
    fn subtract() {
        let (mut b1, mut b2) = (Cell::default(), Cell::default());
        encode(&mut b1, 2);
        encode(&mut b1, 2);
        encode(&mut b2, 1);
        assert_eq!((&b1 - &b2).decode(), Some(Side::Left(1)));
        assert_eq!((&b2 - &b1).decode(), Some(Side::Right(1)));
    }

    #[test]
    fn impure() {
        let mut b1 = Cell::default();
        encode(&mut b1, 1);
        encode(&mut b1, 2);
        assert!(!b1.is_pure());
        assert_eq!(b1.decode(), None);
    }

    #[test]
    fn impure_disjoint() {
        let (mut b1, mut b2) = (Cell::default(), Cell::default());
        encode(&mut b1, 1);
        encode(&mut b1, 2);
        encode(&mut b2, 3);
        assert!(!(&b1 - &b2).is_pure());
    }

    #[test]
    fn zero_element() {
        let mut bucket = Cell::default();
        encode(&mut bucket, 0);
        assert!(!bucket.is_empty());
        assert_eq!(bucket.decode(), Some(Side::Left(0)));
    }
}
