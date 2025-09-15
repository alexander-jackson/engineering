use std::fmt::{self, Debug};
use std::fs::File;
use std::ops::Deref;
use std::path::Path;

use color_eyre::eyre::Result;
use serde::Deserialize;

#[cfg(feature = "external-bytes")]
pub mod external_bytes;

#[cfg(feature = "external-bytes")]
pub use crate::external_bytes::ExternalBytes;

/// Wrapper type to ensure secret values are not displayed.
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

/// Allows arbitrary structs that implement [`Deserialize`] to be read from a file.
pub trait ConfigurationReader: Sized {
    /// Reads the content of the file and attempts to deserialize it into the provided type.
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
    use std::net::Ipv4Addr;

    use color_eyre::eyre::Result;
    use serde::Deserialize;

    use crate::{ConfigurationReader, Secret};

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

    #[test]
    fn can_deserialize_configuration_from_yaml_generically() -> Result<()> {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum ServiceType {
            Internal,
            External,
        }

        #[derive(Deserialize)]
        struct SocketConfiguration {
            addr: Ipv4Addr,
            port: u16,
        }

        #[derive(Deserialize)]
        struct Configuration {
            name: String,
            service_type: ServiceType,
            socket: SocketConfiguration,
        }

        let configuration = Configuration::from_yaml("resources/test-configuration.yaml")?;

        assert_eq!(configuration.name, "foobar");
        assert_eq!(configuration.service_type, ServiceType::Internal);
        assert_eq!(configuration.socket.addr, Ipv4Addr::UNSPECIFIED);
        assert_eq!(configuration.socket.port, 80);

        Ok(())
    }
}
