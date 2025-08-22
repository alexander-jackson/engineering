use std::fmt::{self, Debug};
use std::fs::File;
use std::ops::Deref;
use std::path::Path;

use color_eyre::eyre::Result;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Secret<T>(T);

impl<T> Deref for Secret<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Debug for Secret<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret(...)")
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Secret(value)
    }
}

pub trait ConfigurationReader: Sized {
    fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self>;
}

impl<T> ConfigurationReader for T
where
    T: serde::de::DeserializeOwned,
{
    fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        let reader = File::open(path)?;
        let config = serde_yaml::from_reader(&reader)?;

        tracing::info!(?path, "loaded configuration file");

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;

    use crate::Secret;

    #[test]
    fn secret_values_are_not_displayed() -> Result<()> {
        let secret = Secret("super_secret_value");
        let formatted = format!("{secret:?}");

        assert_eq!(formatted, "Secret(...)");

        Ok(())
    }

    #[test]
    fn can_get_underlying_secret_values() -> Result<()> {
        let hidden = "This is a secret";
        let secret = Secret(hidden);
        let underlying = *secret;

        assert_eq!(underlying, hidden);

        Ok(())
    }
}
