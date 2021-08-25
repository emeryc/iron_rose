//! Iron Rose is a rust implementation of Invertable Bloom Filters as found in
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
//!     estimator.encode(**x);
//! }
//! # for x in core.iter().chain(remote_ids.iter()) {
//! #    remote_estimator.encode(*x);
//! # }
//! // Retreive Remote Estimator in some way
//! let ibf_size = estimator
//!     .estimate_differences(&remote_estimator)
//!     .expect("estimators should be same shape");
//! let mut local = IBF::new(ibf_size);
//! # let mut remote = IBF::new(ibf_size);
//! for x in ids_from_database.iter() {
//!     local.encode(**x);
//! }
//! # for x in core.iter().chain(remote_ids.iter()) {
//! #     remote.encode(*x);
//! # }
//! // Retreive remote IBF
//! let diff = (local - remote).expect("Local and remote should be the same shape");
//! let differences = diff
//!     .decode()
//!     .expect("Successfully decoded because IBFs were large enough");
//!
#![warn(
    missing_docs,
    rust_2018_idioms,
    missing_debug_implementations,
    broken_intra_doc_links
)]
#![allow(clippy::type_complexity)]

mod cell;
mod ibf;
mod strata_estimator;

pub use crate::cell::Side;
pub use ibf::IBF;
pub use strata_estimator::StrataEstimator;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        use super::{Side, StrataEstimator, IBF};
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
        for x in core.iter().chain(local_ids.iter()) {
            estimator.encode(*x);
        }
        for x in core.iter().chain(remote_ids.iter()) {
            remote_estimator.encode(*x);
        }
        // Retreive Remote Estimator in some way
        let ibf_size = estimator
            .estimate_differences(&remote_estimator)
            .expect("estimators should be same shape");
        let mut local = IBF::new(ibf_size * 3);
        let mut remote = IBF::new(ibf_size * 3);
        for x in core.iter().chain(local_ids.iter()) {
            local.encode(*x);
        }
        for x in core.iter().chain(remote_ids.iter()) {
            remote.encode(*x);
        }
        // Retreive remote IBF
        let diff = (local - remote).expect("Local and remote should be the same shape");
        let differences = diff
            .decode()
            .expect("Successfully decoded because IBFs were large enough");

        for x in differences {
            let works = match x {
                Side::Left(local) => local_ids.contains(&local),
                Side::Right(remote) => remote_ids.contains(&remote),
            };
            assert!(works);
        }
    }
}
