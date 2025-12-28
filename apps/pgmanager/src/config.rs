use std::path::PathBuf;

use chrono::NaiveTime;
use foundation_configuration::Secret;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub backup_location: BackupLocation,
    pub backup_schedule: BackupSchedule,
    pub target_database: TargetDatabaseConfiguration,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackupLocation {
    S3 { bucket: String },
    Filesystem { root: PathBuf },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackupSchedule {
    Oneshot,
    Daily { time: NaiveTime },
}

#[derive(Deserialize)]
pub struct TargetDatabaseConfiguration {
    pub username: String,
    pub password: Option<Secret<String>>,
    pub database: String,
    pub host: String,
    pub port: u16,
}

impl From<&TargetDatabaseConfiguration> for tokio_postgres::Config {
    fn from(value: &TargetDatabaseConfiguration) -> Self {
        let mut config = tokio_postgres::Config::new();

        config
            .user(value.username.clone())
            .dbname(value.database.clone())
            .host(value.host.clone())
            .port(value.port);

        if let Some(password) = value.password.as_deref() {
            config.password(password);
        }

        config
    }
}
