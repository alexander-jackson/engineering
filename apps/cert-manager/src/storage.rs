use aws_sdk_s3::primitives::ByteStream;
use color_eyre::eyre::Result;

use crate::configuration::StorageConfiguration;

#[derive(Clone)]
pub struct CertificateStore {
    client: aws_sdk_s3::Client,
    bucket: String,
    prefix: String,
}

impl CertificateStore {
    pub fn new(client: aws_sdk_s3::Client, storage: StorageConfiguration) -> Self {
        Self {
            client,
            bucket: storage.bucket,
            prefix: storage.prefix,
        }
    }

    pub async fn put(&self, domain: &str, private_key: &str, cert_chain: &str) -> Result<()> {
        let prefix = format!("{}/{}", self.prefix, domain);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(format!("{prefix}/fullchain.pem"))
            .body(ByteStream::from(cert_chain.as_bytes().to_vec()))
            .send()
            .await?;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(format!("{prefix}/privkey.pem"))
            .body(ByteStream::from(private_key.as_bytes().to_vec()))
            .send()
            .await?;

        Ok(())
    }
}
