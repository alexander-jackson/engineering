use std::fmt;
use std::ops::Deref;

use serde::Deserialize;

/// The percentage of domains that are classified when first seen, between 0 and 100 inclusive.
///
/// Domains are chosen by hashing their name rather than at random per query, so a domain is
/// either always or never part of the rollout and raising the percentage only ever adds domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize)]
#[serde(try_from = "u8")]
pub struct Rollout(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RolloutError(u8);

impl fmt::Display for RolloutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rollout percentage must be between 0 and 100, got {}",
            self.0
        )
    }
}

impl std::error::Error for RolloutError {}

impl Rollout {
    pub const ALL: Rollout = Rollout(100);

    /// Whether `domain` is part of the rollout.
    pub fn includes(&self, domain: &str) -> bool {
        seahash::hash(domain.as_bytes()) % 100 < u64::from(self.0)
    }
}

impl Default for Rollout {
    fn default() -> Self {
        Self::ALL
    }
}

impl Deref for Rollout {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<u8> for Rollout {
    type Error = RolloutError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value > 100 {
            return Err(RolloutError(value));
        }

        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests {
    use super::{Rollout, RolloutError};

    fn domains() -> impl Iterator<Item = String> {
        (0..200).map(|i| format!("site-{i}.example.com"))
    }

    #[test]
    fn rejects_out_of_range_percentages() {
        assert_eq!(Rollout::try_from(101), Err(RolloutError(101)));
        assert!(Rollout::try_from(0).is_ok());
        assert!(Rollout::try_from(100).is_ok());
    }

    #[test]
    fn zero_includes_nothing_and_hundred_includes_everything() {
        let none = Rollout::try_from(0).unwrap();

        assert!(domains().all(|d| !none.includes(&d)));
        assert!(domains().all(|d| Rollout::ALL.includes(&d)));
    }

    #[test]
    fn includes_roughly_the_requested_share() {
        let rollout = Rollout::try_from(10).unwrap();
        let included = domains().filter(|d| rollout.includes(d)).count();

        // 10% of 200 is 20. The hash is fixed, so this is deterministic, and the bounds are wide
        // enough to not be sensitive to which domains happen to land in the rollout.
        assert!((10..=30).contains(&included), "{included}");
    }

    #[test]
    fn raising_the_percentage_only_adds_domains() {
        let low = Rollout::try_from(5).unwrap();
        let high = Rollout::try_from(25).unwrap();

        assert!(
            domains()
                .filter(|d| low.includes(d))
                .all(|d| high.includes(&d))
        );
    }

    #[test]
    fn deserializes_and_validates() {
        assert_eq!(
            serde_yaml::from_str::<Rollout>("12").unwrap(),
            Rollout::try_from(12).unwrap()
        );
        assert!(serde_yaml::from_str::<Rollout>("101").is_err());
        assert!(serde_yaml::from_str::<Rollout>("-1").is_err());
        assert!(serde_yaml::from_str::<Rollout>("12.5").is_err());
    }
}
