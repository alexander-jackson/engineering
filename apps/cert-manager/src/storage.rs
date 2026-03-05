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
        let (chain_key, privkey_key) = Self::keys(&self.prefix, domain);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(chain_key)
            .body(ByteStream::from(cert_chain.as_bytes().to_vec()))
            .send()
            .await?;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(privkey_key)
            .body(ByteStream::from(private_key.as_bytes().to_vec()))
            .send()
            .await?;

        Ok(())
    }

    fn keys(prefix: &str, domain: &str) -> (String, String) {
        let base = format!("{prefix}/{domain}");
        (
            format!("{base}/fullchain.pem"),
            format!("{base}/privkey.pem"),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::CertificateStore;

    #[test]
    fn keys_builds_correct_paths() {
        let (chain, privkey) = CertificateStore::keys("certs", "example.com");
        assert_eq!(chain, "certs/example.com/fullchain.pem");
        assert_eq!(privkey, "certs/example.com/privkey.pem");
    }

    #[test]
    fn keys_handles_nested_prefix() {
        let (chain, privkey) = CertificateStore::keys("prod/tls", "sub.example.com");
        assert_eq!(chain, "prod/tls/sub.example.com/fullchain.pem");
        assert_eq!(privkey, "prod/tls/sub.example.com/privkey.pem");
    }
}
