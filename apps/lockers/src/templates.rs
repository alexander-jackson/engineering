use axum::body::Body;
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use color_eyre::eyre::Result;
use serde::Serialize;
use tera::{Context, Tera};

use crate::persistence::{BagType, Locker};

#[derive(Clone, Serialize)]
pub struct LockerDisplay {
    pub locker_number: i16,
    pub bag_type: String,
    pub bag_icon: String,
    pub checked_in_ago: String,
}

impl From<Locker> for LockerDisplay {
    fn from(locker: Locker) -> Self {
        let now = Utc::now().naive_local();
        let duration = now.signed_duration_since(locker.checked_in_at);
        let checked_in_ago = format_duration(duration);

        Self {
            locker_number: locker.locker_number,
            bag_type: locker.bag_type.display_name().to_owned(),
            bag_icon: locker.bag_type.icon_path().to_owned(),
            checked_in_ago,
        }
    }
}

fn format_duration(duration: Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;

    if hours > 0 {
        format!("{}h {}m ago", hours, minutes)
    } else if minutes > 0 {
        format!("{}m ago", minutes)
    } else {
        "just now".to_owned()
    }
}

#[derive(Clone, Serialize)]
pub struct IndexContext {
    pub lockers: Vec<LockerDisplay>,
    pub bag_types: Vec<BagTypeDisplay>,
    pub error_message: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct BagTypeDisplay {
    pub value: String,
    pub name: String,
}

impl IndexContext {
    pub fn new(lockers: Vec<Locker>, error_message: Option<String>) -> Self {
        let locker_displays = lockers.into_iter().map(LockerDisplay::from).collect();

        let bag_types = vec![
            BagTypeDisplay {
                value: BagType::PeakDesign30L.as_str().to_owned(),
                name: BagType::PeakDesign30L.display_name().to_owned(),
            },
            BagTypeDisplay {
                value: BagType::StubbleAndCo20L.as_str().to_owned(),
                name: BagType::StubbleAndCo20L.display_name().to_owned(),
            },
        ];

        Self {
            lockers: locker_displays,
            bag_types,
            error_message,
        }
    }
}

#[derive(Clone)]
pub struct TemplateEngine {
    inner: Tera,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let inner = Tera::new("templates/**.tera.html")?;

        Ok(Self { inner })
    }

    pub fn render_serialized<C: Serialize>(
        &self,
        template: &str,
        context: &C,
    ) -> Result<RenderedTemplate> {
        let context = Context::from_serialize(context)?;
        let rendered = self.inner.render(template, &context)?;

        Ok(RenderedTemplate { inner: rendered })
    }
}

pub struct RenderedTemplate {
    inner: String,
}

impl IntoResponse for RenderedTemplate {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/html")
            .header(CACHE_CONTROL, "no-store")
            .body(Body::from(self.inner))
            .unwrap()
    }
}
