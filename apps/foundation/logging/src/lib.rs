use std::str::FromStr;

use tracing::level_filters::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

fn get_env_filter() -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
}

/// The environment variable used to select the output format.
pub const LOG_FORMAT_ENV: &str = "LOG_FORMAT";

/// The format that log events are written in.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Human readable output, intended for local development.
    #[default]
    Pretty,
    /// One JSON object per line, intended for production.
    Json,
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            other => Err(format!("unknown log format: {other:?}")),
        }
    }
}

impl LogFormat {
    /// Reads the format from `LOG_FORMAT`, defaulting to [`LogFormat::Pretty`] when it is unset
    /// or invalid.
    pub fn from_env() -> Self {
        std::env::var(LOG_FORMAT_ENV)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or_default()
    }
}

fn get_fmt_layer<S>(format: LogFormat) -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    get_fmt_layer_with_writer(format, std::io::stdout)
}

fn get_fmt_layer_with_writer<S, W>(
    format: LogFormat,
    writer: W,
) -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    match format {
        LogFormat::Pretty => tracing_subscriber::fmt::layer().with_writer(writer).boxed(),
        LogFormat::Json => tracing_subscriber::fmt::layer()
            .json()
            .with_ansi(false)
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(writer)
            .boxed(),
    }
}

fn add_logging_layers<S>(registry: S, format: LogFormat) -> impl SubscriberExt
where
    S: SubscriberExt + for<'a> LookupSpan<'a> + Send + Sync + 'static,
{
    registry
        .with(get_fmt_layer(format))
        .with(ErrorLayer::default())
        .with(get_env_filter())
}

pub fn install_default_registry() {
    add_logging_layers(Registry::default(), LogFormat::from_env()).init();
}

pub fn get_registry_with_telemetry<L>(telemetry_layer: L) -> impl SubscriberExt
where
    L: Layer<Registry> + Send + Sync + 'static,
{
    let registry = Registry::default().with(telemetry_layer);
    add_logging_layers(registry, LogFormat::from_env())
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::Registry;
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::layer::SubscriberExt;

    use crate::{LogFormat, add_logging_layers, get_fmt_layer_with_writer};

    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Buffer {
        type Writer = Buffer;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn parses_known_formats() {
        assert_eq!("json".parse(), Ok(LogFormat::Json));
        assert_eq!(" JSON ".parse(), Ok(LogFormat::Json));
        assert_eq!("pretty".parse(), Ok(LogFormat::Pretty));
        assert!("xml".parse::<LogFormat>().is_err());
    }

    #[test]
    fn defaults_to_pretty() {
        assert_eq!(LogFormat::default(), LogFormat::Pretty);
    }

    fn capture(format: LogFormat) -> String {
        let buffer = Buffer::default();
        let layer = get_fmt_layer_with_writer(format, buffer.clone());
        let subscriber = Registry::default().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("request", id = 7);
            let _guard = span.enter();
            tracing::info!(user = "alex", "hello");
        });

        String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn json_format_emits_flat_json_with_span_fields() {
        let output = capture(LogFormat::Json);
        let line = output.lines().next().unwrap();

        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(value["message"], "hello");
        assert_eq!(value["user"], "alex");
        assert_eq!(value["level"], "INFO");
        assert_eq!(value["span"]["name"], "request");
        assert_eq!(value["span"]["id"], 7);
        assert_eq!(value["spans"][0]["id"], 7);
        assert!(!line.contains('\u{1b}'), "{line}");
    }

    #[test]
    fn pretty_format_is_not_json() {
        let output = capture(LogFormat::Pretty);

        assert!(serde_json::from_str::<serde_json::Value>(output.trim()).is_err());
        assert!(output.contains("hello"));
    }

    #[test]
    fn both_formats_build_a_subscriber() {
        for format in [LogFormat::Pretty, LogFormat::Json] {
            let _ = add_logging_layers(Registry::default(), format);
        }
    }
}
