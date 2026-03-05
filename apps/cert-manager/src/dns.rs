use std::time::Duration;

use aws_sdk_route53::client::Waiters;
use aws_sdk_route53::types::{
    Change, ChangeAction, ChangeBatch, ResourceRecord, ResourceRecordSet, RrType,
};
use color_eyre::eyre::{Result, eyre};

#[derive(Clone)]
pub struct DnsClient {
    inner: aws_sdk_route53::Client,
}

impl DnsClient {
    pub fn new(inner: aws_sdk_route53::Client) -> Self {
        Self { inner }
    }

    pub async fn set_challenge_record(&self, identifier: &str, value: &str) -> Result<()> {
        let identifier = identifier.to_string();
        let dns_record_name = format!("_acme-challenge.{identifier}");

        tracing::info!(%dns_record_name, ?value, "setting challenge record for domain");

        let zones = self.inner.list_hosted_zones().send().await?.hosted_zones;

        tracing::info!(?zones, "Retrieved hosted zones from Route53");

        let zone = zones
            .iter()
            .find(|zone| identifier.ends_with(zone.name().trim_end_matches('.')))
            .expect("No matching hosted zone found for domain");

        let change = Change::builder()
            .action(ChangeAction::Upsert)
            .resource_record_set(
                ResourceRecordSet::builder()
                    .name(dns_record_name)
                    .r#type(RrType::Txt)
                    .ttl(60)
                    .resource_records(
                        ResourceRecord::builder()
                            .value(format!("\"{value}\""))
                            .build()?,
                    )
                    .build()?,
            )
            .build()?;

        let change_batch = ChangeBatch::builder().changes(change).build()?;

        // Create the DNS record for the challenge and wait for it to propagate
        let response = self
            .inner
            .change_resource_record_sets()
            .hosted_zone_id(zone.id())
            .change_batch(change_batch)
            .send()
            .await?;

        tracing::info!("DNS record created for challenge, waiting for propagation...");

        let id = response
            .change_info
            .as_ref()
            .map(|info| &info.id)
            .ok_or_else(|| eyre!("Failed to get change ID from Route53 response"))?;

        self.inner
            .wait_until_resource_record_sets_changed()
            .id(id)
            .wait(Duration::from_mins(60))
            .await?;

        tracing::info!("DNS record propagated, responding to challenge...");

        Ok(())
    }
}
