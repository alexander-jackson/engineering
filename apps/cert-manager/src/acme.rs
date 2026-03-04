use color_eyre::eyre::{Result, eyre};
use instant_acme::{
    Account, ChallengeType, Identifier, LetsEncrypt, NewAccount, NewOrder, Order, OrderStatus,
    RetryPolicy,
};

#[derive(Clone)]
pub struct AcmeClient {
    account: Account,
}

impl AcmeClient {
    pub async fn new() -> Result<Self> {
        let new_account = NewAccount {
            contact: &["mailto:alexanderjackson@protonmail.com"],
            terms_of_service_agreed: true,
            only_return_existing: false,
        };

        tracing::info!(?new_account, "creating ACME account for certificate renewal");

        let (account, _) = Account::builder()
            .unwrap()
            .create(&new_account, LetsEncrypt::Staging.url().to_owned(), None)
            .await?;

        Ok(Self { account })
    }

    pub async fn create_order(&self, domain: &str) -> Result<(AcmeOrder, Vec<DnsChallenge>)> {
        let identifiers = &[Identifier::Dns(domain.to_owned())];
        let new_order = NewOrder::new(identifiers);

        tracing::info!(?new_order, "creating new ACME order for domain renewal");

        let mut order = self.account.new_order(&new_order).await?;
        let state = order.state();

        tracing::info!(?state, "created new order for certificate renewal");

        let mut dns_challenges = Vec::new();
        let mut authorisations = order.authorizations();

        while let Some(Ok(mut auth)) = authorisations.next().await {
            let challenge = auth
                .challenge(ChallengeType::Dns01)
                .ok_or_else(|| eyre!("no DNS-01 challenge found for authorization"))?;

            dns_challenges.push(DnsChallenge {
                identifier: challenge.identifier().to_string(),
                value: challenge.key_authorization().dns_value(),
            });
        }

        Ok((AcmeOrder { inner: order }, dns_challenges))
    }
}

pub struct DnsChallenge {
    pub identifier: String,
    pub value: String,
}

pub struct AcmeOrder {
    inner: Order,
}

impl AcmeOrder {
    /// Sets all DNS-01 challenges ready, polls until the order is ready,
    /// finalizes, and polls for the certificate.
    /// Returns `(private_key_pem, cert_chain_pem)`.
    pub async fn finalize(mut self) -> Result<(String, String)> {
        let mut authorisations = self.inner.authorizations();

        while let Some(Ok(mut auth)) = authorisations.next().await {
            let mut challenge = auth
                .challenge(ChallengeType::Dns01)
                .ok_or_else(|| eyre!("no DNS-01 challenge found for authorization"))?;

            challenge.set_ready().await?;
        }

        drop(authorisations);

        let retry_policy = RetryPolicy::default();
        tracing::info!(?retry_policy, "Polling order status with retry policy");

        let status = self.inner.poll_ready(&retry_policy).await?;

        match status {
            OrderStatus::Ready => {
                tracing::info!("Order is ready, finalizing...");
                let private_key = self.inner.finalize().await?;
                let cert_chain = self.inner.poll_certificate(&RetryPolicy::default()).await?;

                tracing::info!(
                    "Certificate renewed successfully! Private key length: {}, Certificate chain length: {}",
                    private_key.len(),
                    cert_chain.len()
                );

                Ok((private_key, cert_chain))
            }
            OrderStatus::Valid => {
                tracing::info!("Order is already valid.");
                Err(eyre!("order is already valid; no new certificate was issued"))
            }
            _ => Err(eyre!("unexpected order status after polling: {:?}", status)),
        }
    }
}
