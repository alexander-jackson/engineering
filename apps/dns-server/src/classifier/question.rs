use std::fmt;
use std::ops::Deref;

use serde::Deserialize;

use crate::uid::QuestionUid;

/// A question the decision model scores a domain against, blocking if the score exceeds the
/// threshold.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub uid: QuestionUid,
    pub prompt: Prompt,
    pub threshold: Threshold,
}

/// The text of a question, which is trimmed and never blank.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(try_from = "String")]
pub struct Prompt(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptError {
    Blank,
    TooLong,
}

impl fmt::Display for PromptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptError::Blank => write!(f, "prompt must not be blank"),
            PromptError::TooLong => {
                write!(f, "prompt must be at most {} characters", Prompt::MAX_CHARS)
            }
        }
    }
}

impl std::error::Error for PromptError {}

impl Prompt {
    pub const MAX_CHARS: usize = 1000;
}

impl Deref for Prompt {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<String> for Prompt {
    type Error = PromptError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err(PromptError::Blank);
        }

        if trimmed.chars().count() > Self::MAX_CHARS {
            return Err(PromptError::TooLong);
        }

        Ok(Self(trimmed.to_string()))
    }
}

impl TryFrom<&str> for Prompt {
    type Error = PromptError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.to_string().try_into()
    }
}

impl fmt::Display for Prompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The score above which a question blocks a domain, between 0 and 1 inclusive.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Deserialize)]
#[serde(try_from = "f64")]
pub struct Threshold(f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThresholdError(f64);

impl fmt::Display for ThresholdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "threshold must be between 0 and 1, got {}", self.0)
    }
}

impl std::error::Error for ThresholdError {}

impl Deref for Threshold {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<f64> for Threshold {
    type Error = ThresholdError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        // This also rejects NaN, as it is not contained in any range.
        if !(0.0..=1.0).contains(&value) {
            return Err(ThresholdError(value));
        }

        Ok(Self(value))
    }
}

impl fmt::Display for Threshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::{Prompt, PromptError, Threshold};

    #[test]
    fn prompts_are_trimmed() {
        let prompt = Prompt::try_from("  Is this a proxy?\n").unwrap();

        assert_eq!(&*prompt, "Is this a proxy?");
    }

    #[test]
    fn blank_prompts_are_rejected() {
        assert_eq!(Prompt::try_from(""), Err(PromptError::Blank));
        assert_eq!(Prompt::try_from(" \t\n"), Err(PromptError::Blank));
    }

    #[test]
    fn long_prompts_are_rejected() {
        let at_limit = "é".repeat(Prompt::MAX_CHARS);
        let over_limit = "é".repeat(Prompt::MAX_CHARS + 1);

        assert!(Prompt::try_from(at_limit).is_ok());
        assert_eq!(Prompt::try_from(over_limit), Err(PromptError::TooLong));
    }

    #[test]
    fn thresholds_must_be_between_zero_and_one() {
        assert_eq!(*Threshold::try_from(0.0).unwrap(), 0.0);
        assert_eq!(*Threshold::try_from(0.8).unwrap(), 0.8);
        assert_eq!(*Threshold::try_from(1.0).unwrap(), 1.0);

        for invalid in [-0.1, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Threshold::try_from(invalid).is_err(), "{invalid}");
        }
    }
}
