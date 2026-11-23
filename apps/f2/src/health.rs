use std::time::Duration;

use color_eyre::eyre::Result;
use http_body_util::combinators::BoxBody;
use hyper::body::Bytes;
use hyper::Uri;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use crate::ipc::{HealthCheckRequest, HealthCheckResponse, Receiver, Sender};

const HEALTH_CHECK_PERIOD: Duration = Duration::from_secs(2);
const HEALTH_CHECK_SUCCESS_THRESHOLD: u32 = 3;
const HEALTH_CHECK_FAILURE_THRESHOLD: u32 = 3;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HealthCheckResult {
    Success,
    Failure,
}

pub struct HealthCheckConfiguration {
    period: Duration,
    success_threshold: u32,
    failure_threshold: u32,
}

impl HealthCheckConfiguration {
    pub fn new(period: Duration, success_threshold: u32, failure_threshold: u32) -> Self {
        Self {
            period,
            success_threshold,
            failure_threshold,
        }
    }
}

pub struct HealthCheck {
    target: Uri,
    configuration: HealthCheckConfiguration,
}

impl HealthCheck {
    pub fn new(target: Uri, configuration: HealthCheckConfiguration) -> Self {
        Self {
            target,
            configuration,
        }
    }

    pub async fn run(&self) -> Result<HealthCheckResult> {
        let client: Client<HttpConnector, BoxBody<Bytes, hyper::Error>> =
            Client::builder(TokioExecutor::new()).build_http();

        let mut successes = 0;
        let mut failures = 0;

        loop {
            tokio::time::sleep(self.configuration.period).await;

            match client.get(self.target.clone()).await {
                Ok(res) if res.status().is_success() => successes += 1,
                _ => failures += 1,
            }

            if successes >= self.configuration.success_threshold {
                return Ok(HealthCheckResult::Success);
            }

            if failures >= self.configuration.failure_threshold {
                return Ok(HealthCheckResult::Failure);
            }
        }
    }
}

/// Runs health checks for containers as requested over its inbox.
///
/// Results are logged and sent back as a [`HealthCheckResponse`], it is up to the requester to
/// decide what to do with them.
pub struct HealthMonitor {
    requests: Receiver<HealthCheckRequest>,
    responses: Sender<HealthCheckResponse>,
}

impl HealthMonitor {
    pub fn new(
        requests: Receiver<HealthCheckRequest>,
        responses: Sender<HealthCheckResponse>,
    ) -> Self {
        Self {
            requests,
            responses,
        }
    }

    pub async fn run(&self) -> Result<()> {
        while let Ok(message) = self.requests.recv().await {
            let responses = self.responses.clone();

            // Each check runs in its own task so a slow one doesn't delay the others
            tokio::spawn(async move {
                let request = message.content;
                let configuration = HealthCheckConfiguration::new(
                    HEALTH_CHECK_PERIOD,
                    HEALTH_CHECK_SUCCESS_THRESHOLD,
                    HEALTH_CHECK_FAILURE_THRESHOLD,
                );
                let result = Self::check(&request, configuration).await;

                Self::on_result(&request, &result);
                Self::respond(&responses, &request, result);
            });
        }

        Ok(())
    }

    async fn check(
        request: &HealthCheckRequest,
        configuration: HealthCheckConfiguration,
    ) -> Result<HealthCheckResult> {
        HealthCheck::new(request.target.clone(), configuration)
            .run()
            .await
    }

    /// Reports the outcome to the requester, an errored check counts as a failure.
    fn respond(
        responses: &Sender<HealthCheckResponse>,
        request: &HealthCheckRequest,
        result: Result<HealthCheckResult>,
    ) {
        let response = HealthCheckResponse {
            service: request.service.clone(),
            container_id: request.container_id.clone(),
            result: result.unwrap_or(HealthCheckResult::Failure),
        };

        if let Err(error) = responses.send(response) {
            tracing::warn!(service = %request.service, ?error, "failed to send a health check response");
        }
    }

    fn on_result(request: &HealthCheckRequest, result: &Result<HealthCheckResult>) {
        let HealthCheckRequest {
            service,
            container_id,
            target,
            ..
        } = request;

        match result {
            Ok(HealthCheckResult::Success) => {
                tracing::info!(%service, %container_id, %target, "container reported as healthy");
            }
            Ok(HealthCheckResult::Failure) => {
                tracing::warn!(%service, %container_id, %target, "container reported as unhealthy");
            }
            Err(error) => {
                tracing::warn!(%service, %container_id, %target, ?error, "failed to health check container");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use color_eyre::eyre::{Report, Result};
    use http_body_util::Full;
    use hyper::body::Bytes;
    use hyper::service::service_fn;
    use hyper::{Response, StatusCode, Uri};
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use hyper_util::server::conn::auto::Builder;
    use tokio::net::TcpListener;

    use crate::docker::models::ContainerId;
    use crate::health::{HealthCheck, HealthCheckConfiguration, HealthCheckResult, HealthMonitor};
    use crate::ipc::{channel, HealthCheckRequest};

    async fn spawn_server<F: Fn(u32) -> StatusCode + Copy + Send + Sync + 'static>(
        behaviour: F,
    ) -> Result<SocketAddr> {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
        let listener = TcpListener::bind(&addr).await?;

        let resolved_addr = listener.local_addr()?;
        let requests = Arc::new(AtomicU32::new(1));

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let io = TokioIo::new(stream);

                let requests = Arc::clone(&requests);

                if let Err(err) = Builder::new(TokioExecutor::new())
                    .serve_connection(
                        io,
                        service_fn(move |_| {
                            let requests = Arc::clone(&requests);

                            async move {
                                let status = behaviour(requests.fetch_add(1, Ordering::SeqCst));

                                Ok::<_, Report>(
                                    Response::builder()
                                        .status(status)
                                        .body(Full::<Bytes>::default())
                                        .unwrap(),
                                )
                            }
                        }),
                    )
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            }
        });

        Ok(resolved_addr)
    }

    #[tokio::test]
    async fn can_perform_health_checks() -> Result<()> {
        // service always responds with a 200
        let resolved_addr = spawn_server(|_| StatusCode::OK).await?;

        let target = format!("http://{resolved_addr}");
        let target = Uri::from_str(&target)?;

        let configuration = HealthCheckConfiguration::new(Duration::from_millis(2), 1, 1);
        let health_check = HealthCheck::new(target, configuration);

        let result = health_check.run().await?;

        assert_eq!(result, HealthCheckResult::Success);

        Ok(())
    }

    #[tokio::test]
    async fn health_checks_can_be_failed() -> Result<()> {
        // service always responds with a 500
        let resolved_addr = spawn_server(|_| StatusCode::INTERNAL_SERVER_ERROR).await?;

        let target = format!("http://{resolved_addr}");
        let target = Uri::from_str(&target)?;

        let configuration = HealthCheckConfiguration::new(Duration::from_millis(2), 1, 1);
        let health_check = HealthCheck::new(target, configuration);

        let result = health_check.run().await?;

        assert_eq!(result, HealthCheckResult::Failure);

        Ok(())
    }

    #[tokio::test]
    async fn health_checks_are_retried() -> Result<()> {
        // service responds with 2 500s and then a 200
        let resolved_addr = spawn_server(|requests| match requests {
            3 => StatusCode::OK,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
        .await?;

        let target = format!("http://{resolved_addr}");
        let target = Uri::from_str(&target)?;

        // 3 failures and we fail overall, but a single successful response is enough
        let configuration = HealthCheckConfiguration::new(Duration::from_millis(2), 1, 3);
        let health_check = HealthCheck::new(target, configuration);

        let result = health_check.run().await?;

        assert_eq!(result, HealthCheckResult::Success);

        Ok(())
    }

    #[tokio::test]
    async fn more_complex_health_checks_work_correctly() -> Result<()> {
        // service responds with a 500, then two 200s, then 500s for the rest of time
        let resolved_addr = spawn_server(|requests| match requests {
            2 | 3 => StatusCode::OK,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
        .await?;

        let target = format!("http://{resolved_addr}");
        let target = Uri::from_str(&target)?;

        // Need 3 of either to mark the health check as complete
        let configuration = HealthCheckConfiguration::new(Duration::from_millis(2), 3, 3);
        let health_check = HealthCheck::new(target, configuration);

        let result = health_check.run().await?;

        assert_eq!(result, HealthCheckResult::Failure);

        Ok(())
    }

    #[tokio::test]
    async fn monitor_checks_requested_containers() -> Result<()> {
        // service always responds with a 200
        let resolved_addr = spawn_server(|_| StatusCode::OK).await?;

        let request = HealthCheckRequest {
            service: "foobar".to_owned(),
            container_id: ContainerId("abc123".to_owned()),
            target: Uri::from_str(&format!("http://{resolved_addr}"))?,
        };

        let configuration = HealthCheckConfiguration::new(Duration::from_millis(2), 1, 1);
        let result = HealthMonitor::check(&request, configuration).await?;

        assert_eq!(result, HealthCheckResult::Success);

        Ok(())
    }

    #[tokio::test]
    async fn monitor_responds_to_the_requester() -> Result<()> {
        let (sender, responses) = channel();

        let request = HealthCheckRequest {
            service: "foobar".to_owned(),
            container_id: ContainerId("abc123".to_owned()),
            target: Uri::from_static("http://127.0.0.1:8080/"),
        };

        HealthMonitor::respond(&sender, &request, Ok(HealthCheckResult::Success));
        HealthMonitor::respond(&sender, &request, Err(Report::msg("boom")));

        let first = responses.recv().await?;
        let second = responses.recv().await?;

        assert_eq!(first.content.service, "foobar");
        assert_eq!(first.content.container_id, request.container_id);
        assert_eq!(first.content.result, HealthCheckResult::Success);
        assert_eq!(second.content.result, HealthCheckResult::Failure);

        Ok(())
    }
}
