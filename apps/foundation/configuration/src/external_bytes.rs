use std::path::PathBuf;

use aws_config::BehaviorVersion;
use color_eyre::eyre::{Result, WrapErr, eyre};
use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "location", rename_all = "lowercase")]
pub enum ExternalBytes {
    Filesystem { path: PathBuf },
    S3 { bucket: String, key: String },
}

impl ExternalBytes {
    pub async fn resolve(&self) -> Result<Vec<u8>> {
        let bytes = match self {
            Self::Filesystem { path } => tokio::fs::read(path)
                .await
                .wrap_err_with(|| eyre!("failed to read file at {}", path.display()))?,
            Self::S3 { bucket, key } => {
                let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
                let client = aws_sdk_s3::Client::new(&config);

                let response = client.get_object().bucket(bucket).key(key).send().await?;

                response.body.collect().await?.to_vec()
            }
        };

        tracing::debug!(?self, byte_count = %bytes.len(), "resolved some external bytes");

        Ok(bytes)
    }
}
