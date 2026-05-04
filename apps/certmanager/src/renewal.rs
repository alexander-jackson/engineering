use color_eyre::eyre::{Result, eyre};
use sqlx::types::chrono::{DateTime, Utc};
use x509_parser::pem::Pem;

use crate::acme::AcmeClient;
use crate::configuration::NotifyConfiguration;
use crate::dns::DnsClient;
use crate::storage::CertificateStore;

#[derive(Clone)]
pub struct Renewer {
    acme_client: AcmeClient,
    dns_client: DnsClient,
    cert_store: CertificateStore,
    http_client: reqwest::Client,
    notify: NotifyConfiguration,
}

impl Renewer {
    pub fn new(
        acme_client: AcmeClient,
        dns_client: DnsClient,
        cert_store: CertificateStore,
        http_client: reqwest::Client,
        notify: NotifyConfiguration,
    ) -> Self {
        Self {
            acme_client,
            dns_client,
            cert_store,
            http_client,
            notify,
        }
    }

    pub async fn renew(&self, domain: &str) -> Result<DateTime<Utc>> {
        let (order, challenges) = self.acme_client.create_order(domain).await?;

        for challenge in &challenges {
            self.dns_client
                .set_challenge_record(&challenge.identifier, &challenge.value)
                .await?;
        }

        let (private_key, cert_chain) = order.finalize().await?;
        self.cert_store
            .put(domain, &private_key, &cert_chain)
            .await?;

        let target = format!("{}{}", self.notify.url, self.notify.path);

        match self.http_client.put(&target).send().await {
            Ok(res) if res.status().is_success() => {
                tracing::info!("notified f2 to reload certificates");
            }
            Ok(res) => {
                tracing::warn!(status = %res.status(), "f2 certificate reload returned non-success status");
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to notify f2 to reload certificates");
            }
        }

        let expiry = extract_certificate_expiry(cert_chain.as_bytes())?;

        Ok(expiry)
    }
}

fn extract_certificate_expiry(chain: &[u8]) -> Result<DateTime<Utc>> {
    let mut expiries: Vec<DateTime<Utc>> = Pem::iter_from_buffer(chain)
        .filter_map(Result::ok)
        .filter_map(|pem| {
            let x509 = pem.parse_x509().ok()?;
            let not_after = x509.validity().not_after.timestamp();

            DateTime::from_timestamp(not_after, 0)
        })
        .collect::<Vec<_>>();

    expiries.sort();

    expiries
        .first()
        .cloned()
        .ok_or_else(|| eyre!("No valid certificates found in chain"))
}
