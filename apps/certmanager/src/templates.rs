use serde::Serialize;
use sqlx::types::chrono::Utc;

use crate::persistence::DomainCertificateInfo;

#[derive(Clone, Serialize)]
pub struct DomainDisplay {
    pub domain: String,
    pub expires_at: String,
    pub days_until_expiry: i64,
    pub status: String,
}

impl From<DomainCertificateInfo> for DomainDisplay {
    fn from(info: DomainCertificateInfo) -> Self {
        let now = Utc::now();
        let days_until_expiry = info.expires_at.signed_duration_since(now).num_days();

        let status = if days_until_expiry < 0 {
            "expired".to_owned()
        } else if days_until_expiry <= 7 {
            "expiring-soon".to_owned()
        } else {
            "valid".to_owned()
        };

        Self {
            domain: info.domain,
            expires_at: info.expires_at.format("%b %d, %Y").to_string(),
            days_until_expiry,
            status,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct IndexContext {
    pub domains: Vec<DomainDisplay>,
    pub error_message: Option<String>,
}

impl IndexContext {
    pub fn new(domains: Vec<DomainCertificateInfo>, error_message: Option<String>) -> Self {
        let domain_displays = domains.into_iter().map(DomainDisplay::from).collect();
        Self {
            domains: domain_displays,
            error_message,
        }
    }
}
