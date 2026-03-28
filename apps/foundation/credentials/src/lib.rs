use aws_config::BehaviorVersion;
use aws_config::SdkConfig;
use aws_config::sts::AssumeRoleProvider;
use aws_credential_types::provider::SharedCredentialsProvider;
use color_eyre::eyre::{Result, eyre};

pub async fn load() -> Result<SdkConfig> {
    let current_exe = std::env::current_exe()?;
    let session_name = current_exe
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| eyre!("failed to get current exe file stem"))?
        .to_owned();

    let base_config = aws_config::load_defaults(BehaviorVersion::latest()).await;

    let sdk_config = match std::env::var("AWS_ROLE_ARN") {
        Ok(role_arn) => {
            tracing::info!(%role_arn, %session_name, "AWS_ROLE_ARN set, assuming role");

            let provider = AssumeRoleProvider::builder(role_arn)
                .session_name(session_name)
                .configure(&base_config)
                .build()
                .await;

            base_config
                .into_builder()
                .credentials_provider(SharedCredentialsProvider::new(provider))
                .build()
        }
        Err(_) => {
            tracing::debug!("AWS_ROLE_ARN not set, using default credentials");
            base_config
        }
    };

    Ok(sdk_config)
}
