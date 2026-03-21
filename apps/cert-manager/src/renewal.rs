use color_eyre::eyre::{Result, eyre};
use sqlx::types::chrono::{DateTime, Utc};
use x509_parser::pem::Pem;

use crate::acme::AcmeClient;
use crate::dns::DnsClient;
use crate::storage::CertificateStore;

#[derive(Clone)]
pub struct Renewer {
    acme_client: AcmeClient,
    dns_client: DnsClient,
    cert_store: CertificateStore,
}

impl Renewer {
    pub fn new(
        acme_client: AcmeClient,
        dns_client: DnsClient,
        cert_store: CertificateStore,
    ) -> Self {
        Self {
            acme_client,
            dns_client,
            cert_store,
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
