use std::time::Duration;

use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use chrono::Utc;
use color_eyre::eyre::Result;
use tokio::time::Instant;
use tokio_postgres::NoTls;

mod config;
mod databases;
mod utils;

use crate::config::{BackupLocation, BackupSchedule, Configuration, TargetDatabaseConfiguration};
use crate::databases::{discover, dump};
use crate::utils::{compress, get_initial_offset};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;

    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);

    match config.backup_schedule {
        BackupSchedule::Oneshot => {
            take_backups(&s3_client, &config.backup_location, &config.target_database).await?
        }
        BackupSchedule::Daily { time } => {
            let offset = get_initial_offset(Utc::now().time(), time);
            tracing::info!(?offset, %time, "waiting some time before running the first backups");

            let start = Instant::now() + offset;
            let period = Duration::from_secs(60 * 60 * 24);
            let mut interval = tokio::time::interval_at(start, period);

            loop {
                interval.tick().await;
                take_backups(&s3_client, &config.backup_location, &config.target_database).await?;
            }
        }
    }

    Ok(())
}

async fn take_backups(
    s3_client: &aws_sdk_s3::Client,
    backup_location: &BackupLocation,
    database_config: &TargetDatabaseConfiguration,
) -> Result<()> {
    let date = Utc::now().format("%Y-%m-%d");

    let span = tracing::info_span!("backup", %date);
    let _guard = span.enter();

    let (client, connection) = tokio_postgres::Config::from(database_config)
        .connect(NoTls)
        .await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let databases = discover(&client).await?;

    for database in databases {
        let dump = dump(database_config, &database).await?;
        let compressed = compress(&dump)?;

        let key = format!("{database}/{database}.{date}.sql.gz");

        match backup_location {
            BackupLocation::S3 { bucket } => {
                s3_client
                    .put_object()
                    .bucket(bucket)
                    .key(&key)
                    .body(ByteStream::from(compressed))
                    .send()
                    .await?;
            }
            BackupLocation::Filesystem { root } => {
                let path = root.join(&key);

                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                tokio::fs::write(path, compressed).await?;
            }
        }

        tracing::info!(location = ?backup_location, %key, "persisted a backup");
    }

    Ok(())
}
