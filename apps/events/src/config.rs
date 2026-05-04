use std::net::Ipv4Addr;

use chrono::NaiveDate;
use chrono_tz::Tz;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    pub application: ApplicationConfiguration,
}

#[derive(Deserialize)]
pub struct ServerConfiguration {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub struct ApplicationConfiguration {
    /// The date after which seating events should be considered for display in the history.
    pub seating_cutoff: NaiveDate,
    /// The IANA timezone of the user, used to compute day boundaries (e.g. "Europe/London").
    pub timezone: Tz,
}
