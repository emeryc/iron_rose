use fasthash::metro::hash64;
use serde::{Deserialize, Serialize};

use crate::IBF;

/// Strata Estimator for determining the size of IBF needed to successfuly decode the differences
/// in two sets.
/// ```rust
/// use iron_rose::StrataEstimator;
///
/// let mut se1 = StrataEstimator::default();
/// let mut se2 = StrataEstimator::default();
/// for i in 0..1000 {
///    se1.encode(i);
///    se2.encode(i + 25);
/// }
/// assert_eq!(se1.estimate_differences(&se2), Ok(72));
#[derive(Debug, Serialize, Deserialize)]
pub struct StrataEstimator {
    ibfs: Vec<IBF>,
}

impl Default for StrataEstimator {
    fn default() -> Self {
        Self::new_with_size(32)
    }
}

impl StrataEstimator {
    /// Returns a strata estimator with 32 ibfs allowing you to determin differences as high as
    /// 2^32
    pub fn new_with_size(size: usize) -> Self {
        Self {
            ibfs: (0..size).map(|_| IBF::new(80)).collect::<Vec<_>>(),
        }
    }

    /// Encodes an element into the strata estimator that will eventually to determine the size of
    /// differences between two sets
    pub fn encode(&mut self, element: u128) {
        let trailing = hash64(element.to_be_bytes()).trailing_zeros();
        let len = self.ibfs.len();
        self.ibfs[(trailing as usize % len)].encode(element);
    }

    /// Given another strata estimator, how big of an IBF should you make to successfully
    /// decode the differences provided the IBFs are made of the same elements that went
    /// into these strata estimators.
    pub fn estimate_differences(&self, other: &StrataEstimator) -> Result<usize, String> {
        if self.ibfs.len() != other.ibfs.len() {
            return Err("Strata Estimators are of different sizes".to_string());
        }

        let mut count = 0usize;
        for (i, (l, r)) in self.ibfs.iter().zip(other.ibfs.iter()).enumerate().rev() {
            let ibf = (l - r)?;
            if let Ok(set) = ibf.decode() {
                count += set.len();
            } else {
                count *= 2_usize.pow((i as u32) + 2);
                break;
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic() {
        let mut se1 = StrataEstimator::default();
        let mut se2 = StrataEstimator::default();
        for i in 0..10000 {
            se1.encode(i);
            se2.encode(i + 1000);
        }
        assert!(se1.estimate_differences(&se2).unwrap() > 1000);
    }
}
