//! Hashing used by the IBF and strata estimator.
//!
//! IBFs are meant to be built on one machine and compared on another, so every hash here
//! uses SipHash-1-3 with fixed, hard-coded keys (pure Rust, no platform-specific code), and
//! separate keys for each purpose so the different hashes are independent of each other.
//!
//! Elements are fed through [`std::hash::Hash`], which hands integers to the hasher as
//! native-endian, native-width values. [PortableHasher] converts every integer to fixed-width
//! little-endian bytes before hashing, so hashes are the same on little- and big-endian, and
//! 32- and 64-bit, machines.
//!
//! The one thing that can't be fixed this way: std hashes slices and arrays of multi-byte
//! integers (e.g. `[u32; 4]`) by writing their raw memory in one call, bypassing the
//! per-integer methods. Elements containing those are still endian-dependent.

use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

const CHECKSUM_KEYS: (u64, u64) = (0x6972_6f6e_5f72_6f73, 0x655f_6368_6563_6b73);
const INDEX_KEYS: (u64, u64) = (0x6972_6f6e_5f72_6f73, 0x655f_696e_6465_7820);
const STRATA_KEYS: (u64, u64) = (0x6972_6f6e_5f72_6f73, 0x655f_7374_7261_7461);

/// SipHash-1-3 with fixed keys, which normalises every integer to little-endian bytes (and
/// `usize`/`isize` to 64 bits) before hashing it.
struct PortableHasher(SipHasher13);

macro_rules! le_writes {
    ($($method:ident: $ty:ty),* $(,)?) => {
        $(
            #[inline]
            fn $method(&mut self, n: $ty) {
                self.0.write(&n.to_le_bytes());
            }
        )*
    };
}

impl Hasher for PortableHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }

    le_writes!(
        write_u8: u8, write_u16: u16, write_u32: u32, write_u64: u64, write_u128: u128,
        write_i8: i8, write_i16: i16, write_i32: i32, write_i64: i64, write_i128: i128,
    );

    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }

    #[inline]
    fn write_isize(&mut self, n: isize) {
        self.write_i64(n as i64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }
}

fn hash_with_keys<T: Hash + ?Sized>(element: &T, keys: (u64, u64)) -> u64 {
    let mut hasher = PortableHasher(SipHasher13::new_with_keys(keys.0, keys.1));
    element.hash(&mut hasher);
    hasher.finish()
}

/// Checksum stored in each cell's `hash_sum`, used to detect pure cells.
pub(crate) fn checksum<T: Hash + ?Sized>(element: &T) -> u64 {
    hash_with_keys(element, CHECKSUM_KEYS)
}

/// Base hash used to derive the cell index for each partition (see [partition_hash]).
pub(crate) fn index_hash<T: Hash + ?Sized>(element: &T) -> u64 {
    hash_with_keys(element, INDEX_KEYS)
}

/// Derives the hash for partition `i` from an element's [index_hash], using the splitmix64
/// finalizer. Unlike double hashing (`h1 + i * h2`), this keeps partitions independent:
/// two elements that collide in two partitions are no more likely to collide in a third.
pub(crate) fn partition_hash(base: u64, i: u64) -> u64 {
    let mut z = base.wrapping_add(i.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Hash used by the strata estimator, both to pick a stratum and as the encoded value.
pub(crate) fn strata<T: Hash + ?Sized>(element: &T) -> u64 {
    hash_with_keys(element, STRATA_KEYS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Hash)]
    struct Composite {
        a: u32,
        b: i16,
        c: usize,
        d: (u8, u128),
        e: String,
    }

    fn composite() -> Composite {
        Composite {
            a: 0xDEAD_BEEF,
            b: -2,
            c: 12345,
            d: (7, 0x0102_0304_0506_0708_090A_0B0C_0D0E_0F10),
            e: "iron rose".to_string(),
        }
    }

    /// Hashes are part of the wire format: these must be identical on every platform, and must
    /// never change without a breaking version bump. Run on a big-endian target with
    /// `cargo +nightly miri test --target s390x-unknown-linux-gnu hash::`.
    #[test]
    fn golden_values() {
        let actual = [
            checksum(&0u128),
            checksum(&0x0102_0304_0506_0708_090A_0B0C_0D0E_0F10u128),
            checksum(&42u64),
            index_hash(&42u64),
            strata(&42u64),
            index_hash(&-42i32),
            strata(&usize::MAX),
            checksum("iron rose"),
            checksum(&composite()),
            partition_hash(42, 2),
        ];
        assert_eq!(actual, GOLDEN);
    }

    #[test]
    fn integers_hash_as_little_endian() {
        let raw = |bytes: &[u8]| {
            let mut h = SipHasher13::new_with_keys(CHECKSUM_KEYS.0, CHECKSUM_KEYS.1);
            h.write(bytes);
            h.finish()
        };
        assert_eq!(checksum(&0x0102_0304u32), raw(&[4, 3, 2, 1]));
        assert_eq!(checksum(&-1i16), raw(&[0xff, 0xff]));
        // usize is always hashed as 64 bits, so 32- and 64-bit platforms agree.
        assert_eq!(checksum(&7usize), checksum(&7u64));
        assert_eq!(checksum(&-7isize), checksum(&-7i64));
    }

    /// Generated on aarch64-apple-darwin (little-endian, 64-bit), verified on
    /// s390x-unknown-linux-gnu (big-endian) under Miri.
    const GOLDEN: [u64; 10] = [
        921558936274905133,
        7947040083150445426,
        8772687687399273628,
        7710007203746910558,
        8365220876284598866,
        18411967220398275843,
        10782159099430661292,
        10735287090103526268,
        9449862203432733218,
        5139283748462763858,
    ];
}
