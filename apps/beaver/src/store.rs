use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use color_eyre::eyre::{Result, eyre};
use duckdb::{Config, Connection, Transaction, params};
use jiff::Timestamp;
use serde::Deserialize;
use serde_json::Value;

use crate::configuration::QueryConfiguration;

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
    limits: QueryConfiguration,
}

impl LogStore {
    pub fn new(limits: QueryConfiguration) -> Result<Self> {
        // Queries are user-written, so the database can't reach files or the network
        let config = Config::default().enable_external_access(false)?;
        let connection = Connection::open_in_memory_with_flags(config)?;
        connection.execute_batch(SCHEMA)?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            limits,
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

    /// Runs a user-written `SELECT` against the logs, interrupting it after the configured timeout.
    ///
    /// The query runs on its own connection to the same database, so a slow query never
    /// holds up ingestion.
    pub async fn query(&self, sql: &str) -> Result<QueryResult, QueryError> {
        let sql = sql.trim().trim_end_matches(';').trim_end().to_owned();

        let connection = {
            let connection = self
                .connection
                .lock()
                .map_err(|_| QueryError::Internal(eyre!("connection poisoned")))?;
            connection
                .try_clone()
                .map_err(|error| QueryError::Internal(error.into()))?
        };
        let interrupt = connection.interrupt_handle();

        let limits = self.limits;
        let mut task =
            tokio::task::spawn_blocking(move || run_query(&connection, &sql, limits.max_rows));

        match tokio::time::timeout(limits.timeout(), &mut task).await {
            Ok(result) => result.map_err(|error| QueryError::Internal(error.into()))?,
            Err(_) => {
                // An interrupt sent before DuckDB starts executing is discarded, so keep
                // sending it until the query reacts
                let mut interval = tokio::time::interval(Duration::from_millis(10));

                loop {
                    interrupt.interrupt();

                    tokio::select! {
                        _ = &mut task => break,
                        _ = interval.tick() => {}
                    }
                }

                Err(QueryError::TimedOut(limits.timeout()))
            }
        }
    }
}

/// The outcome of a query, with every value rendered as text and `None` for `NULL`.
#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub truncated: bool,
    pub elapsed: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("{0}")]
    Rejected(String),
    #[error("the query took longer than {} ms and was cancelled", .0.as_millis())]
    TimedOut(Duration),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Internal(color_eyre::Report),
}

fn run_query(
    connection: &Connection,
    sql: &str,
    max_rows: usize,
) -> Result<QueryResult, QueryError> {
    ensure_single_select(connection, sql)?;

    let started = Instant::now();

    // Casting every column to text means values need no per-type handling, and the
    // extra row is how a truncated result is detected
    let wrapped = format!(
        "SELECT COLUMNS(*)::VARCHAR FROM ({sql}) LIMIT {}",
        max_rows + 1
    );
    let mut statement = connection
        .prepare(&wrapped)
        .map_err(|error| QueryError::Invalid(error.to_string()))?;
    let mut rows = statement
        .query([])
        .map_err(|error| QueryError::Invalid(error.to_string()))?;

    let mut columns = Vec::new();
    let mut values = Vec::new();

    while let Some(row) = rows
        .next()
        .map_err(|error| QueryError::Invalid(error.to_string()))?
    {
        if columns.is_empty() {
            columns = row.as_ref().column_names();
        }

        let count = row.as_ref().column_count();
        let cells = (0..count)
            .map(|index| row.get::<_, Option<String>>(index))
            .collect::<duckdb::Result<Vec<_>>>()
            .map_err(|error| QueryError::Invalid(error.to_string()))?;

        values.push(cells);
    }

    // The query asked for one row beyond the limit, so receiving it means the result was cut
    // off. That extra row is only a marker, so it is dropped rather than shown.
    let truncated = values.len() > max_rows;
    values.truncate(max_rows);

    if columns.is_empty() {
        columns = statement.column_names();
    }

    Ok(QueryResult {
        columns,
        rows: values,
        truncated,
        elapsed: started.elapsed(),
    })
}

/// Only a single `SELECT` is allowed, as the logs must not be modified by a query.
///
/// `json_serialize_sql` parses the text without running it and refuses anything but a
/// `SELECT`, which avoids guessing at the statement type from the string.
fn ensure_single_select(connection: &Connection, sql: &str) -> Result<(), QueryError> {
    let serialised: String = connection
        .query_row("SELECT json_serialize_sql(?)", [sql], |row| row.get(0))
        .map_err(|error| QueryError::Internal(error.into()))?;
    let parsed: Value =
        serde_json::from_str(&serialised).map_err(|error| QueryError::Internal(error.into()))?;

    if parsed["error"].as_bool().unwrap_or(false) {
        let message = parsed["error_message"].as_str().unwrap_or("invalid query");

        return Err(if message.contains("Only SELECT") {
            QueryError::Rejected("only SELECT queries are allowed".to_owned())
        } else {
            QueryError::Invalid(message.to_owned())
        });
    }

    match parsed["statements"].as_array().map(Vec::len) {
        Some(1) => Ok(()),
        _ => Err(QueryError::Rejected(
            "only a single statement is allowed".to_owned(),
        )),
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

    use crate::configuration::QueryConfiguration;
    use crate::store::{LogRecord, LogStore, QueryError};

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
        let store = LogStore::new(QueryConfiguration::default())?;
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
        let store = LogStore::new(QueryConfiguration::default())?;
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
        let store = LogStore::new(QueryConfiguration::default())?;
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
        let store = LogStore::new(QueryConfiguration::default())?;

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
        let store = LogStore::new(QueryConfiguration::default())?;
        store.insert(Vec::new()).await?;

        assert_eq!(store.count().await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn queries_return_columns_and_rows_as_text() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        store.insert(vec![record()?]).await?;

        let result = store
            .query("SELECT level, fields->>'$.status' AS status, count(*) AS total FROM logs GROUP BY ALL")
            .await?;

        assert_eq!(result.columns, ["level", "status", "total"]);
        assert_eq!(
            result.rows,
            [[
                Some("INFO".to_owned()),
                Some("200".to_owned()),
                Some("1".to_owned())
            ]]
        );
        assert!(!result.truncated);

        Ok(())
    }

    #[tokio::test]
    async fn queries_return_nulls_as_none() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let mut record = record()?;
        record.level = None;
        store.insert(vec![record]).await?;

        let result = store.query("SELECT level, host FROM logs").await?;

        assert_eq!(result.rows, [[None, Some("primary".to_owned())]]);

        Ok(())
    }

    #[tokio::test]
    async fn queries_keep_columns_when_nothing_matches() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let result = store.query("SELECT host, level FROM logs").await?;

        assert_eq!(result.columns, ["host", "level"]);
        assert!(result.rows.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn queries_accept_a_trailing_semicolon_and_ctes() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        store.insert(vec![record()?]).await?;

        let result = store
            .query("WITH recent AS (SELECT * FROM logs) SELECT count(*) FROM recent;  ")
            .await?;

        assert_eq!(result.rows, [[Some("1".to_owned())]]);

        Ok(())
    }

    #[tokio::test]
    async fn queries_are_cut_off_at_the_row_limit() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;

        let result = store.query("SELECT * FROM range(5000)").await?;

        assert_eq!(result.rows.len(), 1000);
        assert!(result.truncated);

        let result = store.query("SELECT * FROM range(1000)").await?;

        assert_eq!(result.rows.len(), 1000);
        assert!(!result.truncated);

        Ok(())
    }

    #[tokio::test]
    async fn queries_cannot_modify_the_logs() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        store.insert(vec![record()?]).await?;

        for sql in [
            "DROP TABLE logs",
            "DELETE FROM logs",
            "UPDATE logs SET host = 'x'",
            "INSERT INTO logs SELECT * FROM logs",
            "CREATE TABLE other (id INT)",
            "SET enable_external_access = true",
            "ATTACH 'other.db'",
            "COPY logs TO 'out.csv'",
            "SELECT 1; DROP TABLE logs",
            "",
        ] {
            let error = store.query(sql).await.unwrap_err();
            assert!(
                matches!(error, QueryError::Rejected(_) | QueryError::Invalid(_)),
                "{sql:?} gave {error:?}"
            );
        }

        assert_eq!(store.count().await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn queries_cannot_read_files() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;

        for sql in [
            "SELECT * FROM read_csv('Cargo.toml')",
            "SELECT * FROM read_text('Cargo.toml')",
            "SELECT * FROM glob('*')",
        ] {
            let error = store.query(sql).await.unwrap_err();
            assert!(
                matches!(error, QueryError::Invalid(_)),
                "{sql:?} gave {error:?}"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn queries_report_errors_in_the_sql() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;

        let syntax = store.query("SELEC * FROM logs").await.unwrap_err();
        let column = store.query("SELECT nope FROM logs").await.unwrap_err();

        assert!(matches!(syntax, QueryError::Invalid(_)));
        assert!(matches!(column, QueryError::Invalid(message) if message.contains("nope")));

        Ok(())
    }

    #[tokio::test]
    async fn slow_queries_are_cancelled() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let started = std::time::Instant::now();

        let error = store
            .query("SELECT count(*) FROM range(100000000) a, range(100000000) b")
            .await
            .unwrap_err();

        assert!(matches!(error, QueryError::TimedOut(_)), "{error:?}");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));

        Ok(())
    }

    #[tokio::test]
    async fn slow_queries_do_not_block_ingestion() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;

        let slow = {
            let store = store.clone();
            tokio::spawn(async move {
                store
                    .query("SELECT count(*) FROM range(100000000) a, range(100000000) b")
                    .await
            })
        };
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let started = std::time::Instant::now();
        store.insert(vec![record()?]).await?;
        assert!(started.elapsed() < std::time::Duration::from_millis(250));

        assert!(matches!(slow.await?, Err(QueryError::TimedOut(_))));

        Ok(())
    }

    #[tokio::test]
    async fn the_row_limit_is_configurable() -> Result<()> {
        let limits = QueryConfiguration {
            max_rows: 3,
            ..QueryConfiguration::default()
        };
        let store = LogStore::new(limits)?;

        let result = store.query("SELECT * FROM range(10)").await?;

        assert_eq!(result.rows.len(), 3);
        assert!(result.truncated);

        Ok(())
    }

    #[tokio::test]
    async fn the_timeout_is_configurable() -> Result<()> {
        let limits = QueryConfiguration {
            timeout_millis: 50,
            ..QueryConfiguration::default()
        };
        let store = LogStore::new(limits)?;
        let started = std::time::Instant::now();

        let error = store
            .query("SELECT count(*) FROM range(100000000) a, range(100000000) b")
            .await
            .unwrap_err();

        assert!(matches!(error, QueryError::TimedOut(timeout) if timeout.as_millis() == 50));
        assert!(error.to_string().contains("50 ms"));
        assert!(started.elapsed() < std::time::Duration::from_millis(400));

        Ok(())
    }
}
