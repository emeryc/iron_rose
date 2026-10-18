//! Iron Rose is a rust implementation of Invertible Bloom Filters as found in
//! [What's the Difference? Efficient Set Reconciliation without Prior Context](https://www.ics.uci.edu/~eppstein/pubs/EppGooUye-SIGCOMM-11.pdf)
//!
//! Expected use case looks something like this.
//! ```rust
//! use iron_rose::{Side, StrataEstimator, IBF};
//! use uuid::Uuid;
//!
//! let mut estimator = StrataEstimator::default();
//! let mut remote_estimator = StrataEstimator::default();
//! # let core = (0..1000)
//! #     .map(|_| Uuid::new_v4().to_u128_le())
//! #     .collect::<Vec<_>>();
//! # let local_ids = (0..50)
//! #     .map(|_| Uuid::new_v4().to_u128_le())
//! #     .collect::<Vec<_>>();
//! # let remote_ids = (0..50)
//! #     .map(|_| Uuid::new_v4().to_u128_le())
//! #     .collect::<Vec<_>>();
//! # let ids_from_database = core.iter().chain(local_ids.iter()).collect::<Vec<_>>();
//! for x in ids_from_database.iter() {
//!     estimator.encode(*x);
//! }
//! # for x in core.iter().chain(remote_ids.iter()) {
//! #    remote_estimator.encode(x);
//! # }
//! // Retrieve remote estimator in some way
//! let ibf_size = estimator
//!     .recommended_ibf_size(&remote_estimator)
//!     .expect("estimators should be same shape");
//! let mut local = IBF::new(ibf_size);
//! # let mut remote = IBF::new(ibf_size);
//! for x in ids_from_database.iter() {
//!     local.encode(**x);
//! }
//! # for x in core.iter().chain(remote_ids.iter()) {
//! #     remote.encode(*x);
//! # }
//! // Retrieve remote IBF
//! let diff = (local - remote).expect("Local and remote should be the same shape");
//! let differences = diff
//!     .decode()
//!     .expect("Successfully decoded because IBFs were large enough");
//! ```
#![warn(
    missing_docs,
    rust_2018_idioms,
    missing_debug_implementations,
    rustdoc::broken_intra_doc_links
)]

mod cell;
mod error;
mod hash;
mod ibf;
mod strata_estimator;

pub use crate::cell::{Element, Side};
pub use crate::error::{DecodeError, ShapeMismatch};
pub use crate::ibf::{DEFAULT_HASH_COUNT, IBF};
pub use crate::strata_estimator::StrataEstimator;

/// Compiles and runs the README's example as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::{Side, StrataEstimator, IBF};
    use std::collections::HashSet;
    use uuid::Uuid;

    fn ids(n: usize) -> Vec<u128> {
        (0..n).map(|_| Uuid::new_v4().to_u128_le()).collect()
    }

    #[test]
    fn it_works() {
        for _ in 0..20 {
            let (core, local_ids, remote_ids) = (ids(1000), ids(50), ids(50));

            let mut estimator = StrataEstimator::default();
            let mut remote_estimator = StrataEstimator::default();
            for x in core.iter().chain(local_ids.iter()) {
                estimator.encode(x);
            }
            for x in core.iter().chain(remote_ids.iter()) {
                remote_estimator.encode(x);
            }
            let mut ibf_size = estimator
                .recommended_ibf_size(&remote_estimator)
                .expect("estimators should be same shape");

            // Decoding is probabilistic (~99% at the recommended size), so retry with a bigger
            // IBF on failure, as a real caller would.
            let mut attempts = 0;
            let differences = loop {
                let mut local = IBF::new(ibf_size);
                let mut remote = IBF::new(ibf_size);
                for x in core.iter().chain(local_ids.iter()) {
                    local.encode(*x);
                }
                for x in core.iter().chain(remote_ids.iter()) {
                    remote.encode(*x);
                }
                let diff = (local - remote).expect("Local and remote should be the same shape");
                match diff.decode() {
                    Ok(differences) => break differences,
                    Err(e) => {
                        attempts += 1;
                        assert!(attempts < 3, "failed to decode after retries: {}", e);
                        ibf_size *= 2;
                    }
                }
            };

            let expected: HashSet<_> = local_ids
                .iter()
                .map(|x| Side::Left(*x))
                .chain(remote_ids.iter().map(|x| Side::Right(*x)))
                .collect();
            assert_eq!(differences, expected);
        }
    }
}
