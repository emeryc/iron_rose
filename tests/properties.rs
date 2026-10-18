//! Property tests for the public API.
//!
//! Note that "decodes successfully when the IBF is big enough" is deliberately *not* a property:
//! decoding is probabilistic, and with fixed hashes proptest would eventually find (and shrink)
//! an input that fails. That's covered by the seeded statistical tests in `src/ibf.rs` instead.

use iron_rose::{Side, StrataEstimator, IBF};
use proptest::collection::{hash_set, vec};
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashSet;

type Set = HashSet<u128>;

fn ibf_of<'a>(elements: impl IntoIterator<Item = &'a u128>, size: usize, k: usize) -> IBF<u128> {
    let mut ibf = IBF::new_with_hash_count(size, k);
    for x in elements {
        ibf.encode(*x);
    }
    ibf
}

fn symmetric_difference(left: &Set, right: &Set) -> HashSet<Side<u128>> {
    left.difference(right)
        .map(|x| Side::Left(*x))
        .chain(right.difference(left).map(|x| Side::Right(*x)))
        .collect()
}

fn swap(set: HashSet<Side<u128>>) -> HashSet<Side<u128>> {
    set.into_iter()
        .map(|s| match s {
            Side::Left(x) => Side::Right(x),
            Side::Right(x) => Side::Left(x),
        })
        .collect()
}

/// Two sets with some overlap, built from (only left, only right, shared).
fn two_sets() -> impl Strategy<Value = (Set, Set)> {
    (
        hash_set(any::<u128>(), 0..60),
        hash_set(any::<u128>(), 0..60),
        hash_set(any::<u128>(), 0..60),
    )
        .prop_map(|(a, b, common)| {
            let left = a.union(&common).copied().collect();
            let right = b.union(&common).copied().collect();
            (left, right)
        })
}

proptest! {
    /// Whatever decode returns is correct: on success exactly the symmetric difference,
    /// on failure a subset of it.
    #[test]
    fn decode_is_sound((left, right) in two_sets(), size in 0usize..300, k in 1usize..6) {
        let expected = symmetric_difference(&left, &right);
        let diff = (ibf_of(&left, size, k) - ibf_of(&right, size, k)).unwrap();
        match diff.decode() {
            Ok(got) => prop_assert_eq!(got, expected),
            Err(e) => prop_assert!(e.partial.is_subset(&expected)),
        }
    }

    /// Decoding B - A gives the same answer as A - B with the sides swapped.
    #[test]
    fn subtraction_is_antisymmetric((left, right) in two_sets(), size in 0usize..300) {
        let (l, r) = (ibf_of(&left, size, 3), ibf_of(&right, size, 3));
        let lr = (&l - &r).unwrap().decode().map_err(|e| e.partial);
        let rl = (&r - &l).unwrap().decode().map_err(|e| e.partial);
        match (lr, rl) {
            (Ok(a), Ok(b)) => prop_assert_eq!(a, swap(b)),
            (Err(a), Err(b)) => prop_assert_eq!(a, swap(b)),
            (a, b) => prop_assert!(false, "one side decoded and the other didn't: {:?} / {:?}", a, b),
        }
    }

    /// Elements common to both sides cancel out completely, so the shared part doesn't matter.
    #[test]
    fn common_elements_cancel(
        (left, right) in two_sets(),
        common in hash_set(any::<u128>(), 0..60),
        size in 0usize..300,
    ) {
        let with_common = |s: &Set| s.union(&common).copied().collect::<Set>();
        let plain = (ibf_of(&left, size, 3) - ibf_of(&right, size, 3)).unwrap();
        let padded = (ibf_of(&with_common(&left), size, 3) - ibf_of(&with_common(&right), size, 3)).unwrap();
        // Only valid when `common` doesn't overlap the differences, which would change them.
        let overlaps = common.iter().any(|x| left.contains(x) != right.contains(x));
        prop_assume!(!overlaps);
        prop_assert_eq!(plain, padded);
    }

    /// A - A is empty, and decodes to nothing.
    #[test]
    fn self_subtraction_is_empty(set in hash_set(any::<u128>(), 0..200), size in 0usize..300) {
        let ibf = ibf_of(&set, size, 3);
        let diff = (&ibf - &ibf).unwrap();
        prop_assert!(diff.is_empty());
        prop_assert!(diff.decode().unwrap().is_empty());
    }

    /// The order elements are encoded in doesn't matter.
    #[test]
    fn encoding_order_is_irrelevant(elements in vec(any::<u128>(), 0..200), size in 0usize..300) {
        let forward = ibf_of(&elements, size, 3);
        let backward = ibf_of(elements.iter().rev(), size, 3);
        prop_assert_eq!(forward, backward);
    }

    /// IBFs of different shapes refuse to subtract instead of panicking.
    #[test]
    fn shape_mismatch_is_an_error(a in 0usize..100, b in 0usize..100, ka in 1usize..6, kb in 1usize..6) {
        let (x, y) = (IBF::<u64>::new_with_hash_count(a, ka), IBF::<u64>::new_with_hash_count(b, kb));
        let same = x.cell_count() == y.cell_count() && ka == kb;
        prop_assert_eq!((&x - &y).is_ok(), same);
    }

    #[test]
    fn serde_round_trip(set in hash_set(any::<u128>(), 0..100), size in 0usize..200, k in 1usize..6) {
        let ibf = ibf_of(&set, size, k);
        let back: IBF<u128> = serde_json::from_str(&serde_json::to_string(&ibf).unwrap()).unwrap();
        prop_assert_eq!(ibf, back);
    }

    /// IBFs from an untrusted peer: arbitrary (possibly hostile) cells are either rejected on
    /// deserialization, or can be used without panicking or hanging.
    #[test]
    fn hostile_ibfs_never_panic(
        cells in vec(hostile_cell(), 0..40),
        hash_count in 0usize..6,
        extra in vec(any::<u64>(), 0..20),
    ) {
        let cells: Vec<_> = cells
            .into_iter()
            .map(|(id, hash, count)| json!({"id_sum": id, "hash_sum": hash, "count": count}))
            .collect();
        let json = json!({"cells": cells, "hash_count": hash_count});
        if let Ok(mut ibf) = serde_json::from_value::<IBF<u64>>(json) {
            let honest = IBF::new_with_hash_count(ibf.cell_count(), ibf.hash_count());
            let _ = (&ibf - &ibf).unwrap().decode();
            let _ = (&honest - &ibf).unwrap().decode();
            let _ = (&ibf - &honest).unwrap().decode();
            for x in extra {
                ibf.encode(x);
            }
            let _ = ibf.decode();
        }
    }

    /// Identical sets always estimate to zero differences.
    #[test]
    fn estimator_identical_sets(set in hash_set(any::<u128>(), 0..300)) {
        let mut a = StrataEstimator::default();
        let mut b = StrataEstimator::default();
        for x in &set {
            a.encode(x);
            b.encode(x);
        }
        prop_assert_eq!(a.estimate_differences(&b), Ok(0));
    }

    #[test]
    fn estimator_shape_mismatch(a in 1usize..40, b in 1usize..40) {
        let (x, y) = (StrataEstimator::new_with_size(a), StrataEstimator::new_with_size(b));
        prop_assert_eq!(x.estimate_differences(&y).is_ok(), a == b);
    }
}

/// The `(id_sum, hash_sum, count)` of a cell holding only `x`, with the real checksum.
/// Extracted via serde since the checksum isn't public.
fn pure_cell(x: u64) -> (u64, u64, i64) {
    let mut ibf = IBF::new_with_hash_count(1, 1);
    ibf.encode(x);
    let json = serde_json::to_value(&ibf).unwrap();
    (x, json["cells"][0]["hash_sum"].as_u64().unwrap(), 1)
}

/// Junk cells, extreme counts, and cells that look pure but may be in the wrong place.
fn hostile_cell() -> impl Strategy<Value = (u64, u64, i64)> {
    prop_oneof![
        (any::<u64>(), any::<u64>(), any::<i64>()),
        (
            any::<u64>(),
            any::<u64>(),
            prop_oneof![Just(i64::MIN), Just(i64::MAX), Just(1), Just(-1)]
        ),
        (any::<u64>(), any::<bool>()).prop_map(|(x, neg)| {
            let (id, hash, _) = pure_cell(x);
            (id, hash, if neg { -1 } else { 1 })
        }),
        Just((0, 0, 0)),
    ]
}

/// Regression: a hostile count of i64::MIN used to overflow (and panic in debug builds).
#[test]
fn extreme_counts_dont_overflow() {
    let cell = json!({"id_sum": 1, "hash_sum": 2, "count": i64::MIN});
    let json = json!({"cells": [cell.clone(), cell.clone(), cell], "hash_count": 3});
    let hostile: IBF<u64> = serde_json::from_value(json).unwrap();
    let mut honest = IBF::new(3);
    honest.encode(5);
    assert!((&honest - &hostile).unwrap().decode().is_err());
}

/// Regression: a cell that looks pure, but isn't in one of its element's own slots, used to
/// make decode loop forever, since peeling it never emptied it.
#[test]
fn misplaced_pure_cells_dont_hang() {
    for x in 0..50u64 {
        let (id, hash, count) = pure_cell(x);
        let cell = json!({"id_sum": id, "hash_sum": hash, "count": count});
        let empty = json!({"id_sum": 0, "hash_sum": 0, "count": 0});
        // Put the pure cell in every slot of a 1-cell-per-partition IBF; one of those is
        // right, the others are misplaced.
        for slot in 0..3 {
            let mut cells = vec![empty.clone(); 3];
            cells[slot] = cell.clone();
            let ibf: IBF<u64> =
                serde_json::from_value(json!({"cells": cells, "hash_count": 3})).unwrap();
            let _ = ibf.decode();
        }
    }
}
