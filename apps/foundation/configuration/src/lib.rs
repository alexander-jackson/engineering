use std::fmt::{self, Debug};
use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use color_eyre::eyre::Result;
use serde::Deserialize;
use serde_yaml::Value;

#[cfg(feature = "external-bytes")]
pub mod external_bytes;

#[cfg(feature = "external-bytes")]
pub use crate::external_bytes::ExternalBytes;

/// Wrapper type to ensure secret values are not displayed.
#[derive(Clone, Deserialize, PartialEq, Eq)]
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
    ///
    /// If a sibling override file exists (`config.yaml` -> `config.override.yaml`), it is
    /// deep-merged on top of the base file before deserializing. Override files are intended
    /// to be git-ignored, so developers can keep personal values and secrets out of the
    /// committed configuration.
    fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self>;
}

/// Computes the path of the override file for a configuration file.
fn override_path(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    let extension = path.extension()?.to_str()?;

    Some(path.with_file_name(format!("{stem}.override.{extension}")))
}

/// Recursively merges `overlay` into `base`. Mappings are merged key by key, any other value
/// in the overlay replaces the one in the base.
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Mapping(base), Value::Mapping(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

impl<T> ConfigurationReader for T
where
    T: serde::de::DeserializeOwned,
{
    fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        let overlay = override_path(path).filter(|p| p.is_file());

        let config = match overlay {
            // No override, deserialize directly to keep precise error locations
            None => serde_yaml::from_reader(File::open(path)?)?,
            Some(overlay) => {
                let mut base: Value = serde_yaml::from_reader(File::open(path)?)?;
                let overlay_value: Value = serde_yaml::from_reader(File::open(&overlay)?)?;

                merge(&mut base, overlay_value);

                tracing::info!(path = %overlay.display(), "applied configuration override");

                serde_yaml::from_value(base)?
            }
        };

        tracing::info!(path = %path.display(), "loaded configuration file");

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use color_eyre::eyre::Result;
    use serde::Deserialize;

    use std::path::{Path, PathBuf};

    use crate::{ConfigurationReader, Secret, override_path};

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
    fn override_path_inserts_suffix_before_extension() {
        let path = override_path(Path::new("apps/today/local-config.yaml"));

        assert_eq!(
            path,
            Some(PathBuf::from("apps/today/local-config.override.yaml"))
        );
    }

    #[test]
    fn override_files_are_deep_merged_over_the_base() -> Result<()> {
        #[derive(Deserialize)]
        struct Database {
            host: String,
            password: Secret<String>,
        }

        #[derive(Deserialize)]
        struct Configuration {
            name: String,
            database: Database,
        }

        let dir = std::env::temp_dir().join(format!("foundation-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;

        let base = dir.join("config.yaml");
        std::fs::write(
            &base,
            "name: base\ndatabase:\n  host: localhost\n  password: ''\n",
        )?;
        std::fs::write(
            dir.join("config.override.yaml"),
            "database:\n  password: hunter2\n",
        )?;

        let configuration = Configuration::from_yaml(&base)?;
        std::fs::remove_dir_all(&dir)?;

        assert_eq!(configuration.name, "base");
        assert_eq!(configuration.database.host, "localhost");
        assert_eq!(*configuration.database.password, "hunter2");

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
