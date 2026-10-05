use std::net::Ipv4Addr;

use chrono::Weekday;
use foundation_configuration::Secret;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfig,
    /// Externally reachable base URL, used to build links sent over Telegram
    pub base_url: String,
    pub schedule: ScheduleConfig,
    pub openrouter: OpenRouterConfig,
    pub telegram: TelegramConfig,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub host: Ipv4Addr,
    pub port: u16,
}

/// When the weekly job runs, in UTC
#[derive(Copy, Clone, Deserialize)]
pub struct ScheduleConfig {
    pub weekday: Weekday,
    pub hour: u32,
    pub minute: u32,
}

#[derive(Deserialize)]
pub struct OpenRouterConfig {
    pub api_key: Secret<String>,
    pub model: String,
    #[serde(default = "default_openrouter_base_url")]
    pub base_url: String,
}

#[derive(Deserialize)]
pub struct TelegramConfig {
    pub bot_token: Secret<String>,
    /// Quoted in YAML, as channel identifiers are often negative numbers
    pub chat_id: String,
    #[serde(default = "default_telegram_base_url")]
    pub base_url: String,
}

fn default_openrouter_base_url() -> String {
    "https://openrouter.ai/api/v1".to_owned()
}

fn default_telegram_base_url() -> String {
    "https://api.telegram.org".to_owned()
}
