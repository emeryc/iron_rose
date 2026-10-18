# Iron Rose: (Invertible Bloom Filter for Rust)

## What is it?

Iron Rose is a rust implementation of Invertible Bloom Filters as found in
[What's the Difference? Efficient Set Reconciliation without Prior Context](https://www.ics.uci.edu/~eppstein/pubs/EppGooUye-SIGCOMM-11.pdf)

## Expected use
```rust
use iron_rose::{Side, StrataEstimator, IBF};
use uuid::Uuid;

let mut estimator = StrataEstimator::default();
let mut remote_estimator = StrataEstimator::default();
let core = (0..1000)
    .map(|_| Uuid::new_v4().to_u128_le())
    .collect::<Vec<_>>();
let local_ids = (0..50)
    .map(|_| Uuid::new_v4().to_u128_le())
    .collect::<Vec<_>>();
let remote_ids = (0..50)
    .map(|_| Uuid::new_v4().to_u128_le())
    .collect::<Vec<_>>();
let ids_from_database = core.iter().chain(local_ids.iter()).collect::<Vec<_>>();
for x in ids_from_database.iter() {
    estimator.encode(*x);
}
for x in core.iter().chain(remote_ids.iter()) {
    remote_estimator.encode(x);
}
// Retrieve remote estimator in some way
let ibf_size = estimator
    .recommended_ibf_size(&remote_estimator)
    .expect("estimators should be same shape");
let mut local = IBF::new(ibf_size);
let mut remote = IBF::new(ibf_size);
for x in ids_from_database.iter() {
    local.encode(**x);
}
for x in core.iter().chain(remote_ids.iter()) {
    remote.encode(*x);
}
// Retrieve remote IBF
let diff = (local - remote).expect("Local and remote should be the same shape");
let differences = diff
    .decode()
    .expect("Successfully decoded because IBFs were large enough");
```

## Worthwhile Notes

* Anything that implements `Clone + Hash + BitXor + BitXorAssign + Default + Eq + Debug` (the
  `Element` trait, implemented automatically) can be stored in an IBF, and if it also implements
  serde's `Serialize`/`Deserialize` then so does the IBF. This means we get the benefits of the
  basic IBF idea, but can encode larger and more complex things than just IDs.
* Decoding is probabilistic. At `StrataEstimator::recommended_ibf_size` (2x the estimated
  difference + 30 cells) decoding succeeds ~99% of the time; on failure `decode` returns a
  `DecodeError` holding the partially decoded set, and you should retry with a larger IBF.
* Hashing uses SipHash-1-3 with fixed keys, with every integer normalised to 64-bit-or-wider
  little-endian bytes, so IBFs built on different machines (including big-endian and 32-bit
  ones) are compatible. The exception is elements containing arrays or slices of multi-byte
  integers (e.g. `[u32; 4]`), which std hashes as raw native-endian memory.
* Each IBF's cells are split into `hash_count` partitions, with each element stored in one cell
  of each partition, so every element always lands in `hash_count` distinct cells.

Future enhancements "I plan to add"™ are wrappers around various basic types that allow us to
send slices back and forth relatively easily.

## Changes in 0.2

0.2 is not wire-compatible with 0.1 (the hash function changed), and has some API changes:

* `fasthash` (which doesn't build on non-x86 platforms) was replaced with `siphasher`.
* Fixed elements sometimes hashing to the same cell more than once, which made decoding fail far
  more often than it should.
* `StrataEstimator::estimate_differences` now returns an (unbiased) estimate of the number of
  differences, rather than an IBF size that overestimated by 2-4x. Use the new
  `recommended_ibf_size` to size IBFs.
* `StrataEstimator::encode` takes `&T` (so it also accepts unsized types like `str`).
* Errors are now `ShapeMismatch` and `DecodeError<T>` instead of `String`.
* `IBF::new(size)` rounds `size` up to a non-zero multiple of the hash count.
* Deserialized IBFs and strata estimators are validated, instead of panicking on use, and
  `decode` can't panic or loop forever on hostile cell contents.
* Hashing is endian- and word-size-independent (see above).
* Property tests (proptest) replace the libfuzzer target.
* `decode` is now O(cells + differences) instead of O(cells × differences).
