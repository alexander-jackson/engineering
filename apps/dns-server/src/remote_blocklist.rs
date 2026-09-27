use std::collections::HashSet;

use color_eyre::eyre::{Context, Result, eyre};

/// Downloads and parses a remote blocklist in the HaGeZi "Wildcard Domains" format.
#[tracing::instrument]
pub async fn fetch(url: &str) -> Result<HashSet<String>> {
    tracing::info!("fetching remote blocklist");

    let body = reqwest::get(url)
        .await
        .wrap_err_with(|| format!("failed to fetch remote blocklist from {url}"))?
        .error_for_status()
        .wrap_err_with(|| format!("remote blocklist request to {url} failed"))?
        .text()
        .await
        .wrap_err_with(|| format!("failed to read remote blocklist body from {url}"))?;

    let domains = wildcard_domains_parser::parse(&body)
        .map_err(|e| eyre!("failed to parse remote blocklist from {url}: {e}"))?;

    tracing::info!(count = domains.len(), "fetched remote blocklist");

    Ok(domains.into_iter().collect())
}
