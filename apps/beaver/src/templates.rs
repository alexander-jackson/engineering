use serde::Serialize;

use crate::store::QueryResult;

/// A query shown beneath the form, to save writing the common ones out.
struct Example {
    title: &'static str,
    sql: &'static str,
}

const EXAMPLES: [Example; 4] = [
    Example {
        title: "Recent logs",
        sql: "SELECT timestamp, image_name, level, message\nFROM logs\nORDER BY timestamp DESC\nLIMIT 100",
    },
    Example {
        title: "Recent errors",
        sql: "SELECT timestamp, image_name, target, message\nFROM logs\nWHERE level = 'ERROR'\nORDER BY timestamp DESC\nLIMIT 100",
    },
    Example {
        title: "Logs per application",
        sql: "SELECT image_name, level, count(*) AS logs\nFROM logs\nGROUP BY ALL\nORDER BY logs DESC",
    },
    Example {
        title: "Requests by status",
        sql: "SELECT image_name, fields->>'$.status' AS status, count(*) AS requests\nFROM logs\nWHERE fields->>'$.status' IS NOT NULL\nGROUP BY ALL\nORDER BY requests DESC",
    },
];

/// The `logs` table as documented, shown on the page so queries can be written without the docs.
const SCHEMA: [(&str, &str, &str); 12] = [
    ("timestamp", "TIMESTAMPTZ", "when vector collected the log"),
    ("image_name", "VARCHAR", "the application, without its tag"),
    ("image_tag", "VARCHAR", ""),
    ("container_name", "VARCHAR", "changes on every roll"),
    ("container_id", "VARCHAR", "distinguishes instances"),
    ("container_created", "TIMESTAMPTZ", ""),
    ("host", "VARCHAR", ""),
    ("stream", "VARCHAR", "stdout or stderr"),
    ("level", "VARCHAR", "upper case, null for plain text"),
    ("target", "VARCHAR", ""),
    ("message", "VARCHAR", ""),
    ("fields", "JSON", "everything else the application logged"),
];

#[derive(Serialize)]
struct ExampleContext {
    title: &'static str,
    href: String,
}

#[derive(Serialize)]
struct ColumnContext {
    name: &'static str,
    kind: &'static str,
    note: &'static str,
}

#[derive(Serialize)]
struct CellContext {
    null: bool,
    value: String,
}

#[derive(Serialize)]
struct ResultContext {
    columns: Vec<String>,
    rows: Vec<Vec<CellContext>>,
    row_count: usize,
    truncated: bool,
    elapsed_ms: u128,
}

#[derive(Serialize)]
pub struct IndexContext {
    query: String,
    total_rows: String,
    examples: Vec<ExampleContext>,
    schema: Vec<ColumnContext>,
    result: Option<ResultContext>,
    error: Option<String>,
}

impl IndexContext {
    pub fn new(
        query: Option<String>,
        total_rows: i64,
        outcome: Option<Result<QueryResult, String>>,
    ) -> Self {
        let (result, error) = match outcome {
            Some(Ok(result)) => (Some(ResultContext::from(result)), None),
            Some(Err(error)) => (None, Some(error)),
            None => (None, None),
        };

        Self {
            query: query.unwrap_or_else(|| EXAMPLES[0].sql.to_owned()),
            total_rows: format_count(total_rows),
            examples: EXAMPLES
                .iter()
                .map(|example| ExampleContext {
                    title: example.title,
                    href: format!("/?{}", encode_query(example.sql)),
                })
                .collect(),
            schema: SCHEMA
                .iter()
                .map(|&(name, kind, note)| ColumnContext { name, kind, note })
                .collect(),
            result,
            error,
        }
    }
}

impl From<QueryResult> for ResultContext {
    fn from(result: QueryResult) -> Self {
        let rows = result
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|cell| CellContext {
                        null: cell.is_none(),
                        value: cell.unwrap_or_default(),
                    })
                    .collect()
            })
            .collect::<Vec<_>>();

        Self {
            columns: result.columns,
            row_count: rows.len(),
            rows,
            truncated: result.truncated,
            elapsed_ms: result.elapsed.as_millis(),
        }
    }
}

fn encode_query(sql: &str) -> String {
    serde_urlencoded::to_string([("q", sql)]).unwrap_or_default()
}

/// Groups digits in threes, so `1204` reads as `1,204 rows`.
fn format_count(count: i64) -> String {
    let digits = count.to_string();
    let mut grouped = String::new();

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }

        grouped.push(digit);
    }

    format!("{grouped} {}", if count == 1 { "row" } else { "rows" })
}

#[cfg(test)]
mod tests {
    use crate::templates::format_count;

    #[test]
    fn counts_are_grouped_and_pluralised() {
        assert_eq!(format_count(0), "0 rows");
        assert_eq!(format_count(1), "1 row");
        assert_eq!(format_count(999), "999 rows");
        assert_eq!(format_count(1204), "1,204 rows");
        assert_eq!(format_count(1_234_567), "1,234,567 rows");
    }
}
