use axum::Router;
use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::Response;
use axum::routing::{get, post};
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use chrono_tz::Tz;
use color_eyre::eyre::{Result, eyre};
use foundation_http_server::Server;
use foundation_templating::{RenderedTemplate, TemplateEngine};
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::config::ApplicationConfiguration;
use crate::error::ServerResult;
use crate::persistence::EventType;
use crate::templates::{HistoryContext, IndexContext};

#[derive(Debug, Default)]
enum OccurredAt {
    #[default]
    Now,
    Explicit(NaiveDateTime),
}

impl<'de> Deserialize<'de> for OccurredAt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        if s.is_empty() {
            return Ok(OccurredAt::Now);
        }
        NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M")
            .map(OccurredAt::Explicit)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
struct EventForm {
    #[serde(default)]
    occurred_at: OccurredAt,
    #[serde(default)]
    timezone: String,
}

impl EventForm {
    fn into_utc(self) -> Result<DateTime<Utc>> {
        match self.occurred_at {
            OccurredAt::Now => Ok(Utc::now()),
            OccurredAt::Explicit(naive) => {
                if self.timezone.is_empty() {
                    return Ok(naive.and_utc());
                }
                let tz: Tz = self
                    .timezone
                    .parse()
                    .map_err(|_| eyre!("unknown timezone: {:?}", self.timezone))?;
                naive
                    .and_local_timezone(tz)
                    .earliest()
                    .map(|dt| dt.to_utc())
                    .ok_or_else(|| {
                        eyre!(
                            "datetime {:?} does not exist in timezone {:?}",
                            naive,
                            self.timezone
                        )
                    })
            }
        }
    }
}

#[derive(Clone)]
struct ApplicationState {
    configuration: ApplicationConfiguration,
    template_engine: TemplateEngine,
    pool: PgPool,
}

pub fn build(
    configuration: ApplicationConfiguration,
    template_engine: TemplateEngine,
    pool: PgPool,
    listener: TcpListener,
) -> Server {
    let state = ApplicationState {
        configuration,
        template_engine,
        pool,
    };

    let router = Router::new()
        .route("/", get(index))
        .route("/history", get(history))
        .route("/insert", post(insert))
        .route("/remove", post(remove))
        .route("/seating", post(seating))
        .with_state(state);

    Server::new(router, listener)
}

#[tracing::instrument(skip(template_engine, pool))]
async fn index(
    State(ApplicationState {
        template_engine,
        pool,
        ..
    }): State<ApplicationState>,
) -> ServerResult<RenderedTemplate> {
    let today_start = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    let stats = crate::persistence::get_daily_stats(&pool, today_start, Utc::now()).await?;
    let context = IndexContext::from(stats);
    let rendered = template_engine.render_serialized("index.tera.html", &context)?;

    Ok(rendered)
}

#[tracing::instrument(skip(template_engine, pool))]
async fn history(
    State(ApplicationState {
        configuration,
        template_engine,
        pool,
        ..
    }): State<ApplicationState>,
) -> ServerResult<RenderedTemplate> {
    let days = crate::persistence::get_history(&pool, Utc::now()).await?;
    let context = HistoryContext::from(days, configuration.seating_cutoff);
    let rendered = template_engine.render_serialized("history.tera.html", &context)?;

    Ok(rendered)
}

#[tracing::instrument(skip(pool))]
async fn insert(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
    Form(form): Form<EventForm>,
) -> ServerResult<Response> {
    crate::persistence::record_event(&pool, EventType::Inserted, form.into_utc()?).await?;
    tracing::info!("recorded insert event");
    Ok(redirect("/")?)
}

#[tracing::instrument(skip(pool))]
async fn remove(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
    Form(form): Form<EventForm>,
) -> ServerResult<Response> {
    crate::persistence::record_event(&pool, EventType::Removed, form.into_utc()?).await?;
    tracing::info!("recorded remove event");
    Ok(redirect("/")?)
}

#[tracing::instrument(skip(pool))]
async fn seating(
    State(ApplicationState { pool, .. }): State<ApplicationState>,
    Form(form): Form<EventForm>,
) -> ServerResult<Response> {
    crate::persistence::record_seating(&pool, form.into_utc()?).await?;
    tracing::info!("recorded seating");
    Ok(redirect("/")?)
}

fn redirect(path: &'static str) -> Result<Response> {
    let res = Response::builder()
        .status(StatusCode::FOUND)
        .header(LOCATION, path)
        .body(Body::empty())?;

    Ok(res)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, Utc};
    use serde::Deserialize;
    use serde::de::IntoDeserializer;

    use super::{EventForm, OccurredAt};

    fn parse_naive(s: &str) -> Result<OccurredAt, serde::de::value::Error> {
        OccurredAt::deserialize(s.into_deserializer())
    }

    fn to_utc(occurred_at: &str, timezone: &str) -> color_eyre::eyre::Result<DateTime<Utc>> {
        let form = EventForm {
            occurred_at: parse_naive(occurred_at).map_err(|e| color_eyre::eyre::eyre!("{e}"))?,
            timezone: timezone.to_string(),
        };
        form.into_utc()
    }

    #[test]
    fn empty_string_falls_back_to_now() {
        let before = Utc::now();
        let result = to_utc("", "").unwrap();
        let after = Utc::now();
        assert!(result >= before && result <= after);
    }

    #[test]
    fn empty_timezone_with_explicit_time_falls_back_to_utc() {
        let result = to_utc("2026-03-18T09:30", "").unwrap();
        assert_eq!(result.to_rfc3339(), "2026-03-18T09:30:00+00:00");
    }

    #[test]
    fn invalid_datetime_returns_error() {
        assert!(parse_naive("not-a-date").is_err());
    }

    #[test]
    fn parsed_time_has_no_seconds() {
        let result = to_utc("2026-03-18T09:30", "").unwrap();
        assert_eq!(result.timestamp() % 60, 0);
    }

    #[test]
    fn past_datetime_is_before_now() {
        let result = to_utc("2020-01-01T00:00", "").unwrap();
        assert!(result < Utc::now() - Duration::days(365));
    }

    #[test]
    fn london_summer_time_converted_to_utc() {
        // Europe/London is UTC+1 in summer (BST)
        let result = to_utc("2026-07-01T10:00", "Europe/London").unwrap();
        assert_eq!(result.to_rfc3339(), "2026-07-01T09:00:00+00:00");
    }

    #[test]
    fn london_winter_time_converted_to_utc() {
        // Europe/London is UTC+0 in winter (GMT)
        let result = to_utc("2026-01-01T10:00", "Europe/London").unwrap();
        assert_eq!(result.to_rfc3339(), "2026-01-01T10:00:00+00:00");
    }

    #[test]
    fn new_york_winter_time_converted_to_utc() {
        // America/New_York is UTC-5 in winter (EST)
        let result = to_utc("2026-01-01T10:00", "America/New_York").unwrap();
        assert_eq!(result.to_rfc3339(), "2026-01-01T15:00:00+00:00");
    }

    #[test]
    fn invalid_timezone_returns_error() {
        assert!(to_utc("2026-01-01T10:00", "Fake/Timezone").is_err());
    }
}
