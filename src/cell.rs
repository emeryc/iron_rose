use fasthash::{HasherExt, Murmur3HasherExt as ElmHasher};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, BitXor, BitXorAssign, Deref, Sub, SubAssign};

/// Which side of the IBF is this from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    /// Was on the "Left" side and missing in the "Right" side
    Left(T),
    /// Was on the "Right" side and missing in the "Left" side
    Right(T),
}

impl<T> Deref for Side<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Side::Left(l) => l,
            Side::Right(r) => r,
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, Default)]
pub(crate) struct Cell<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    // Are my sums the right size?
    id_sum: T,
    hash_sum: u128,
    count: i32,
}
impl<T> Cell<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    pub(crate) fn encode(&mut self, element: T) {
        let mut hasher: ElmHasher = Default::default();
        element.hash(&mut hasher);

        self.id_sum ^= element;
        self.hash_sum ^= hasher.finish_ext();
        self.count += 1;
    }

    pub(crate) fn is_pure(&self) -> bool {
        let mut hasher: ElmHasher = Default::default();
        self.id_sum.hash(&mut hasher);

        (self.count == 1 || self.count == -1) && self.hash_sum == hasher.finish_ext()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0 && self.hash_sum == 0 && self.id_sum == Default::default()
    }

    pub(crate) fn decode(&self) -> Result<Side<T>, String> {
        if !self.is_pure() {
            return Err("Impure bucket".to_string());
        }
        Ok(if self.count == 1 {
            Side::Left(self.id_sum.clone())
        } else {
            Side::Right(self.id_sum.clone())
        })
    }
}

impl<T> Add for Cell<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    type Output = Cell<T>;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            id_sum: self.id_sum ^ rhs.id_sum,
            hash_sum: self.hash_sum ^ rhs.hash_sum,
            count: self.count + rhs.count,
        }
    }
}
impl<T> SubAssign for Cell<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.id_sum ^= rhs.id_sum;
        self.hash_sum ^= rhs.hash_sum;
        self.count -= rhs.count;
    }
}

impl<T> Sub for Cell<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    type Output = Cell<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            id_sum: self.id_sum ^ rhs.id_sum,
            hash_sum: self.hash_sum ^ rhs.hash_sum,
            count: self.count - rhs.count,
        }
    }
}

impl<T> Sub for &Cell<T>
where
    T: Clone
        + std::hash::Hash
        + BitXor<Output = T>
        + BitXorAssign
        + Default
        + PartialEq
        + Eq
        + Debug,
{
    type Output = Cell<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        Cell {
            id_sum: self.id_sum.clone() ^ rhs.id_sum.clone(),
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
        let mut bucket: Cell<u128> = Default::default();
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
