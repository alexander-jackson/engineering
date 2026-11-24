use std::sync::{Arc, Mutex};

use color_eyre::eyre::{Result, eyre};
use duckdb::{Connection, Transaction, params};
use jiff::Timestamp;
use serde::Deserialize;
use serde_json::Value;

const SCHEMA: &str = "
    CREATE TABLE logs (
        timestamp          TIMESTAMPTZ NOT NULL,
        image_name         VARCHAR     NOT NULL,
        image_tag          VARCHAR     NOT NULL,
        container_name     VARCHAR     NOT NULL,
        container_id       VARCHAR     NOT NULL,
        container_created  TIMESTAMPTZ NOT NULL,
        host               VARCHAR     NOT NULL,
        stream             VARCHAR     NOT NULL,
        level              VARCHAR,
        target             VARCHAR,
        message            VARCHAR,
        fields             JSON
    )
";

const INSERT: &str = "
    INSERT INTO logs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?::JSON)
";

/// A single log in the shape `vector` sends it, matching the `logs` table.
#[derive(Debug, Deserialize)]
pub struct LogRecord {
    pub timestamp: Timestamp,
    pub image_name: String,
    pub image_tag: String,
    pub container_name: String,
    pub container_id: String,
    pub container_created: Timestamp,
    pub host: String,
    pub stream: String,
    pub level: Option<String>,
    pub target: Option<String>,
    pub message: Option<String>,
    pub fields: Value,
}

/// An in-memory DuckDB database holding logs until they are flushed to S3.
#[derive(Clone)]
pub struct LogStore {
    connection: Arc<Mutex<Connection>>,
}

impl LogStore {
    pub fn new() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(SCHEMA)?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Inserts a batch atomically, on a blocking thread as DuckDB is synchronous.
    pub async fn insert(&self, records: Vec<LogRecord>) -> Result<()> {
        let connection = Arc::clone(&self.connection);

        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut connection = connection
                .lock()
                .map_err(|_| eyre!("connection poisoned"))?;
            let transaction = connection.transaction()?;

            insert_records(&transaction, &records)?;
            transaction.commit()?;
            Ok(())
        })
        .await?
    }

    #[cfg(test)]
    pub async fn count(&self) -> Result<i64> {
        let connection = Arc::clone(&self.connection);

        tokio::task::spawn_blocking(move || -> Result<i64> {
            let connection = connection
                .lock()
                .map_err(|_| eyre!("connection poisoned"))?;
            let count = connection.query_row("SELECT count(*) FROM logs", [], |row| row.get(0))?;
            Ok(count)
        })
        .await?
    }
}

fn insert_records(transaction: &Transaction, records: &[LogRecord]) -> Result<()> {
    let mut statement = transaction.prepare_cached(INSERT)?;

    for record in records {
        statement.execute(params![
            record.timestamp.to_string(),
            record.image_name,
            record.image_tag,
            record.container_name,
            record.container_id,
            record.container_created.to_string(),
            record.host,
            record.stream,
            record.level,
            record.target,
            record.message,
            record.fields.to_string(),
        ])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;
    use jiff::Timestamp;
    use serde_json::json;

    use crate::store::{LogRecord, LogStore};

    fn timestamp(value: &str) -> Result<Timestamp> {
        Ok(value.parse()?)
    }

    fn record() -> Result<LogRecord> {
        Ok(LogRecord {
            timestamp: timestamp("2026-11-24T10:00:00.123Z")?,
            image_name: "alexanderjackson/dns-server".to_owned(),
            image_tag: "20261121-0830".to_owned(),
            container_name: "happy_turing".to_owned(),
            container_id: "abc123".to_owned(),
            container_created: timestamp("2026-11-24T09:00:00Z")?,
            host: "primary".to_owned(),
            stream: "stdout".to_owned(),
            level: Some("INFO".to_owned()),
            target: Some("dns_server::resolver".to_owned()),
            message: Some("resolved".to_owned()),
            fields: json!({"user": "alex", "status": 200, "span": {"name": "query"}}),
        })
    }

    fn query<T: duckdb::types::FromSql>(store: &LogStore, sql: &str) -> Result<T> {
        let connection = store.connection.lock().unwrap();
        Ok(connection.query_row(sql, [], |row| row.get(0))?)
    }

    #[tokio::test]
    async fn stores_every_column() -> Result<()> {
        let store = LogStore::new()?;
        let record = record()?;
        let (timestamp, created) = (record.timestamp, record.container_created);
        store.insert(vec![record]).await?;

        let text = |column: &str| query::<String>(&store, &format!("SELECT {column} FROM logs"));

        assert_eq!(text("image_name")?, "alexanderjackson/dns-server");
        assert_eq!(text("image_tag")?, "20261121-0830");
        assert_eq!(text("container_name")?, "happy_turing");
        assert_eq!(text("container_id")?, "abc123");
        assert_eq!(text("host")?, "primary");
        assert_eq!(text("stream")?, "stdout");
        assert_eq!(text("level")?, "INFO");
        assert_eq!(text("target")?, "dns_server::resolver");
        assert_eq!(text("message")?, "resolved");

        let millis =
            |column: &str| query::<i64>(&store, &format!("SELECT epoch_ms({column}) FROM logs"));
        assert_eq!(millis("timestamp")?, timestamp.as_millisecond());
        assert_eq!(millis("container_created")?, created.as_millisecond());

        Ok(())
    }

    #[tokio::test]
    async fn stores_fields_as_queryable_json() -> Result<()> {
        let store = LogStore::new()?;
        store.insert(vec![record()?]).await?;

        let user = query::<String>(&store, "SELECT fields->>'$.user' FROM logs")?;
        let status = query::<i64>(&store, "SELECT (fields->>'$.status')::BIGINT FROM logs")?;
        let span = query::<String>(&store, "SELECT fields->>'$.span.name' FROM logs")?;
        let column_type = query::<String>(&store, "SELECT typeof(fields) FROM logs")?;

        assert_eq!(user, "alex");
        assert_eq!(status, 200);
        assert_eq!(span, "query");
        assert_eq!(column_type, "JSON");

        Ok(())
    }

    #[tokio::test]
    async fn stores_missing_optional_values_as_null() -> Result<()> {
        let store = LogStore::new()?;
        let mut record = record()?;
        record.level = None;
        record.target = None;
        record.message = None;
        record.fields = json!({});
        store.insert(vec![record]).await?;

        let nulls = query::<i64>(
            &store,
            "SELECT count(*) FROM logs WHERE level IS NULL AND target IS NULL AND message IS NULL",
        )?;
        let fields = query::<String>(&store, "SELECT fields::VARCHAR FROM logs")?;

        assert_eq!(nulls, 1);
        assert_eq!(fields, "{}");

        Ok(())
    }

    #[tokio::test]
    async fn accumulates_batches_in_order() -> Result<()> {
        let store = LogStore::new()?;

        let batch = |messages: &[&str]| -> Result<Vec<LogRecord>> {
            messages
                .iter()
                .map(|message| {
                    let mut record = record()?;
                    record.message = Some((*message).to_owned());
                    Ok(record)
                })
                .collect()
        };

        store.insert(batch(&["one", "two"])?).await?;
        store.insert(batch(&["three"])?).await?;

        let messages = query::<String>(
            &store,
            "SELECT string_agg(message, ',' ORDER BY rowid) FROM logs",
        )?;

        assert_eq!(store.count().await?, 3);
        assert_eq!(messages, "one,two,three");

        Ok(())
    }

    #[tokio::test]
    async fn accepts_an_empty_batch() -> Result<()> {
        let store = LogStore::new()?;
        store.insert(Vec::new()).await?;

        assert_eq!(store.count().await?, 0);

        Ok(())
    }
}
