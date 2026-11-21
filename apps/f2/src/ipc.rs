use std::sync::Arc;

use color_eyre::eyre::{eyre, Result};
use flume::{Receiver, Sender};
use hyper::Uri;
use uuid::Uuid;

use crate::docker::models::ContainerId;

pub struct Message<T> {
    pub identifier: Uuid,
    pub content: T,
}

#[derive(Debug)]
pub struct CertificateUpdateRequest;
#[derive(Debug)]
pub struct ReconciliationRequest;

/// A request to health check a newly started container.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HealthCheckRequest {
    pub service: String,
    pub container_id: ContainerId,
    pub target: Uri,
}

#[derive(Debug)]
pub struct ChannelPair<T> {
    sender: Sender<Message<T>>,
    receiver: Receiver<Message<T>>,
}

impl<T> ChannelPair<T> {
    pub fn new() -> Self {
        let (sender, receiver) = flume::unbounded();

        ChannelPair { sender, receiver }
    }
}

#[derive(Debug)]
pub struct MessageBus {
    reconciliation: ChannelPair<ReconciliationRequest>,
    resolver: ChannelPair<CertificateUpdateRequest>,
    health: ChannelPair<HealthCheckRequest>,
}

impl MessageBus {
    pub fn new() -> Arc<Self> {
        let reconciliation_pair = ChannelPair::<ReconciliationRequest>::new();
        let resolver_pair = ChannelPair::<CertificateUpdateRequest>::new();
        let health_pair = ChannelPair::<HealthCheckRequest>::new();

        let message_bus = MessageBus {
            reconciliation: reconciliation_pair,
            resolver: resolver_pair,
            health: health_pair,
        };

        Arc::new(message_bus)
    }

    pub fn send_reconciliation_request(&self) -> Result<Uuid> {
        let identifier = Uuid::new_v4();
        let message = Message {
            identifier,
            content: ReconciliationRequest,
        };

        tracing::debug!(%identifier, "sending reconciliation request");

        self.reconciliation
            .sender
            .send(message)
            .map_err(|_| eyre!("Failed to send reconciliation request"))?;

        Ok(identifier)
    }

    pub fn send_certificate_update_request(&self) -> Result<Uuid> {
        let identifier = Uuid::new_v4();
        let message = Message {
            identifier,
            content: CertificateUpdateRequest,
        };

        tracing::debug!(%identifier, "sending certificate update request");

        self.resolver
            .sender
            .send(message)
            .map_err(|_| eyre!("Failed to send certificate update request"))?;

        Ok(identifier)
    }

    pub fn send_health_check_request(&self, request: HealthCheckRequest) -> Result<Uuid> {
        let identifier = Uuid::new_v4();
        let message = Message {
            identifier,
            content: request,
        };

        tracing::debug!(%identifier, "sending health check request");

        self.health
            .sender
            .send(message)
            .map_err(|_| eyre!("Failed to send health check request"))?;

        Ok(identifier)
    }

    pub async fn receive_reconciliation_request(
        &self,
    ) -> Result<Message<ReconciliationRequest>, flume::RecvError> {
        let received = self.reconciliation.receiver.recv_async().await?;

        tracing::debug!(%received.identifier, "received reconciliation request");

        Ok(received)
    }

    pub async fn receive_certificate_update_request(
        &self,
    ) -> Result<Message<CertificateUpdateRequest>, flume::RecvError> {
        let received = self.resolver.receiver.recv_async().await?;

        tracing::debug!(%received.identifier, "received certificate update request");

        Ok(received)
    }

    pub async fn receive_health_check_request(
        &self,
    ) -> Result<Message<HealthCheckRequest>, flume::RecvError> {
        let received = self.health.receiver.recv_async().await?;

        tracing::debug!(%received.identifier, "received health check request");

        Ok(received)
    }
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;

    use hyper::Uri;

    use crate::docker::models::ContainerId;
    use crate::ipc::{HealthCheckRequest, MessageBus};

    #[tokio::test]
    async fn can_send_and_receive_reconciliation_requests() -> Result<()> {
        let message_bus = MessageBus::new();

        let sent = message_bus.send_reconciliation_request()?;
        let received = message_bus.receive_reconciliation_request().await?;

        assert_eq!(sent, received.identifier);

        Ok(())
    }

    #[tokio::test]
    async fn can_send_and_receive_certificate_update_requests() -> Result<()> {
        let message_bus = MessageBus::new();

        let sent = message_bus.send_certificate_update_request()?;
        let received = message_bus.receive_certificate_update_request().await?;

        assert_eq!(sent, received.identifier);

        Ok(())
    }

    #[tokio::test]
    async fn can_send_and_receive_health_check_requests() -> Result<()> {
        let message_bus = MessageBus::new();

        let request = HealthCheckRequest {
            service: "foobar".to_owned(),
            container_id: ContainerId("abc123".to_owned()),
            target: Uri::from_static("http://127.0.0.1:8080/"),
        };

        let sent = message_bus.send_health_check_request(request.clone())?;
        let received = message_bus.receive_health_check_request().await?;

        assert_eq!(sent, received.identifier);
        assert_eq!(received.content, request);

        Ok(())
    }
}
