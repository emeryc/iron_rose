use crate::error::ShapeMismatch;
use crate::{hash, IBF};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::hash::Hash;

/// Number of cells in each stratum's IBF, as per the paper.
const STRATUM_CELLS: usize = 80;

/// Strata Estimator for estimating the number of differences between two sets, so you can
/// size an [IBF] large enough to decode them.
/// ```rust
/// use iron_rose::StrataEstimator;
///
/// let mut se1 = StrataEstimator::default();
/// let mut se2 = StrataEstimator::default();
/// for i in 0..1000u64 {
///    se1.encode(&i);
///    se2.encode(&(i + 25));
/// }
/// // The true difference is 50
/// assert_eq!(se1.estimate_differences(&se2), Ok(50));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawStrataEstimator")]
pub struct StrataEstimator {
    ibfs: Vec<IBF<u64>>,
}

#[derive(Deserialize)]
struct RawStrataEstimator {
    ibfs: Vec<IBF<u64>>,
}

impl TryFrom<RawStrataEstimator> for StrataEstimator {
    type Error = &'static str;

    fn try_from(raw: RawStrataEstimator) -> Result<Self, Self::Error> {
        if raw.ibfs.is_empty() {
            return Err("StrataEstimator must have at least one stratum");
        }
        Ok(StrataEstimator { ibfs: raw.ibfs })
    }
}

impl Default for StrataEstimator {
    /// A strata estimator with 32 strata, able to estimate differences up to roughly 2^32.
    fn default() -> Self {
        Self::new_with_size(32)
    }
}

impl StrataEstimator {
    /// Returns a strata estimator with `size` strata, allowing you to estimate differences
    /// as high as roughly 2^`size`.
    ///
    /// # Panics
    /// If `size` is 0.
    pub fn new_with_size(size: usize) -> Self {
        assert!(size > 0, "StrataEstimator needs at least one stratum");
        Self {
            ibfs: (0..size).map(|_| IBF::new(STRATUM_CELLS)).collect(),
        }
    }

    /// Encodes an element into the strata estimator. Encode the same elements you'll later
    /// put into your [IBF].
    pub fn encode<T: Hash + ?Sized>(&mut self, element: &T) {
        let h = hash::strata(element);
        // Stratum i receives ~1/2^(i+1) of all elements.
        let stratum = (h.trailing_zeros() as usize).min(self.ibfs.len() - 1);
        self.ibfs[stratum].encode(h);
    }

    /// Estimates the number of elements in the symmetric difference of the two sets encoded
    /// into these estimators.
    ///
    /// This is an *estimate*, and can be somewhat lower than the true value; use
    /// [recommended_ibf_size](Self::recommended_ibf_size) to size an IBF.
    pub fn estimate_differences(&self, other: &StrataEstimator) -> Result<usize, ShapeMismatch> {
        if self.ibfs.len() != other.ibfs.len() {
            return Err(ShapeMismatch);
        }

        let mut count = 0usize;
        for (i, (l, r)) in self.ibfs.iter().zip(other.ibfs.iter()).enumerate().rev() {
            match (l - r)?.decode() {
                Ok(set) => count += set.len(),
                // Strata 0..=i couldn't be decoded. Everything above i holds ~1/2^(i+1) of the
                // differences, so scale up to account for the rest.
                Err(_) => {
                    let scale = 1usize.checked_shl(i as u32 + 1).unwrap_or(usize::MAX);
                    return Ok(count.max(1).saturating_mul(scale));
                }
            }
        }
        Ok(count)
    }

    /// Suggested number of cells for an [IBF] holding the differences between these two sets:
    /// `2 * estimate + 30`, which adds headroom on top of
    /// [estimate_differences](Self::estimate_differences) both for the IBF's own overhead and
    /// for estimation error.
    ///
    /// IBFs of this size decode successfully ~99% of the time. Decoding is probabilistic, so
    /// callers should still handle a [DecodeError](crate::DecodeError), e.g. by retrying with
    /// a larger IBF.
    pub fn recommended_ibf_size(&self, other: &StrataEstimator) -> Result<usize, ShapeMismatch> {
        let estimate = self.estimate_differences(other)?;
        Ok(estimate.saturating_mul(2).saturating_add(30))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn estimators(common: u64, diff: u64) -> (StrataEstimator, StrataEstimator) {
        let mut se1 = StrataEstimator::default();
        let mut se2 = StrataEstimator::default();
        for i in 0..common {
            se1.encode(&i);
            se2.encode(&i);
        }
        for i in common..common + diff {
            if i % 2 == 0 {
                se1.encode(&i);
            } else {
                se2.encode(&i);
            }
        }
        (se1, se2)
    }

    #[test]
    fn identical_sets() {
        let (se1, se2) = estimators(10_000, 0);
        assert_eq!(se1.estimate_differences(&se2), Ok(0));
    }

    #[test]
    fn small_differences_are_exact() {
        // Small differences decode in every stratum, so the count is exact.
        let (se1, se2) = estimators(10_000, 40);
        assert_eq!(se1.estimate_differences(&se2), Ok(40));
    }

    #[test]
    fn large_differences_are_close() {
        for &d in &[500u64, 2_000, 10_000, 50_000] {
            let (se1, se2) = estimators(10_000, d);
            let est = se1.estimate_differences(&se2).unwrap() as f64;
            let ratio = est / d as f64;
            assert!((0.5..2.0).contains(&ratio), "d={} est={}", d, est);
        }
    }

    #[test]
    fn shape_mismatch() {
        let se1 = StrataEstimator::new_with_size(16);
        let se2 = StrataEstimator::new_with_size(32);
        assert_eq!(se1.estimate_differences(&se2), Err(ShapeMismatch));
    }

    #[test]
    fn single_stratum() {
        let mut se1 = StrataEstimator::new_with_size(1);
        let se2 = StrataEstimator::new_with_size(1);
        for i in 0..10u64 {
            se1.encode(&i);
        }
        assert_eq!(se1.estimate_differences(&se2), Ok(10));
    }

    #[test]
    fn serde_rejects_empty() {
        assert!(serde_json::from_str::<StrataEstimator>(r#"{"ibfs":[]}"#).is_err());
    }
}
