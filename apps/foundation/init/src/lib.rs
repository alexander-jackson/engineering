use std::ops::Deref;

use color_eyre::eyre::{Result, eyre};
use foundation_args::Args;
use foundation_configuration::ConfigurationReader;
use foundation_telemetry::TelemetryConfig;
use serde::Deserialize;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration<T> {
    #[serde(flatten)]
    pub application: T,
    pub telemetry: Option<TelemetryConfig>,
}

impl<T> Deref for Configuration<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.application
    }
}

pub fn run<T>() -> Result<Configuration<T>>
where
    T: for<'de> Deserialize<'de>,
{
    color_eyre::install()?;

    let current_exe = std::env::current_exe()?;

    let application_name = current_exe
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| eyre!("failed to get current exe file stem"))?
        .to_owned();

    let args = Args::from_env()?;
    let config = Configuration::from_yaml(&args.config)?;

    let registry = foundation_logging::get_default_registry();

    match &config.telemetry {
        Some(telemetry) if telemetry.enabled => {
            let layer = foundation_telemetry::get_trace_layer(
                application_name.clone(),
                &telemetry.endpoint,
            )?;
            registry.with(layer).init();
        }
        _ => {
            registry.init();
        }
    }

    tracing::info!(name = %application_name, "initialised application");

    Ok(config)
}
