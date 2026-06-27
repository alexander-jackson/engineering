use std::process::Stdio;

use color_eyre::eyre::{Context, Result, eyre};
use tokio::process::Command;
use tokio_postgres::Client;

use crate::config::TargetDatabaseConfiguration;

pub struct TableSize {
    pub schema: String,
    pub table: String,
    pub bytes: u64,
    pub row_count: u64,
}

#[tracing::instrument(skip(client), ret)]
pub async fn discover(client: &Client) -> Result<Vec<String>> {
    let query = r#"
        SELECT datname
        FROM pg_database
        WHERE datname NOT IN (
            'postgres',
            'template0',
            'template1',
            'rdsadmin'
        )
    "#;

    let rows = client.query(query, &[]).await?;
    let databases = rows.into_iter().map(|row| row.get(0)).collect();

    tracing::info!(?databases, "discovered some targets for backup");

    Ok(databases)
}

#[tracing::instrument(skip(client))]
pub async fn table_sizes(client: &Client) -> Result<Vec<TableSize>> {
    let rows = client
        .query(
            r#"
                SELECT
                    schemaname,
                    relname,
                    pg_total_relation_size(schemaname || '.' || relname),
                    n_live_tup
                FROM pg_stat_user_tables
            "#,
            &[],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| TableSize {
            schema: r.get(0),
            table: r.get(1),
            bytes: r.get::<_, i64>(2) as u64,
            row_count: r.get::<_, i64>(3) as u64,
        })
        .collect())
}

#[tracing::instrument(skip(config))]
pub async fn dump(config: &TargetDatabaseConfiguration, database: &str) -> Result<Vec<u8>> {
    let mut command = Command::new("pg_dump");

    command
        .args([
            "-h",
            &config.host,
            "-p",
            &config.port.to_string(),
            "-d",
            database,
            "-U",
            &config.username,
        ])
        .stdout(Stdio::piped());

    if config.ssl {
        command.env("PGSSLMODE", "require");
    }

    if let Some(password) = config.password.as_deref() {
        command.env("PGPASSWORD", password);
    }

    let stdout = command
        .spawn()
        .wrap_err_with(|| eyre!("failed to run `pg_dump` command"))?
        .wait_with_output()
        .await?
        .stdout;

    tracing::info!(bytes = %stdout.len(), "got some output from the dump");

    Ok(stdout)
}
