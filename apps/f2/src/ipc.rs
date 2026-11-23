//! Typed channels for components of `f2` to talk to each other without calling each other.
//!
//! Every component owns the [`Receiver`] for the messages it accepts, and hands out [`Sender`]s
//! to whoever is allowed to talk to it. Answers travel back over a channel of their own.

use std::any::type_name;
use std::fmt;

use color_eyre::eyre::{eyre, Result};
use hyper::Uri;
use uuid::Uuid;

use crate::docker::models::ContainerId;
use crate::health::HealthCheckResult;

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

/// The outcome of a [`HealthCheckRequest`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HealthCheckResponse {
    pub service: String,
    pub container_id: ContainerId,
    pub result: HealthCheckResult,
}

pub struct Sender<T> {
    inner: flume::Sender<Message<T>>,
}

pub struct Receiver<T> {
    inner: flume::Receiver<Message<T>>,
}

/// Creates an unbounded channel for messages of type `T`.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (sender, receiver) = flume::unbounded();

    (Sender { inner: sender }, Receiver { inner: receiver })
}

impl<T> Sender<T> {
    pub fn send(&self, content: T) -> Result<Uuid> {
        let identifier = Uuid::new_v4();
        let message = Message {
            identifier,
            content,
        };

        tracing::debug!(%identifier, message = type_name::<T>(), "sending message");

        self.inner
            .send(message)
            .map_err(|_| eyre!("Failed to send {}", type_name::<T>()))?;

        Ok(identifier)
    }
}

impl<T> Receiver<T> {
    /// Waits for the next message, erroring once every [`Sender`] has been dropped.
    pub async fn recv(&self) -> Result<Message<T>, flume::RecvError> {
        let received = self.inner.recv_async().await?;

        tracing::debug!(
            identifier = %received.identifier,
            message = type_name::<T>(),
            "received message",
        );

        Ok(received)
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sender<{}>", type_name::<T>())
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Receiver<{}>", type_name::<T>())
    }
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;

    use crate::ipc::{channel, ReconciliationRequest};

    #[tokio::test]
    async fn can_send_and_receive_messages() -> Result<()> {
        let (sender, receiver) = channel();

        let sent = sender.send("hello")?;
        let received = receiver.recv().await?;

        assert_eq!(sent, received.identifier);
        assert_eq!(received.content, "hello");

        Ok(())
    }

    #[tokio::test]
    async fn messages_are_received_in_the_order_they_were_sent() -> Result<()> {
        let (sender, receiver) = channel();

        sender.send(1)?;
        sender.clone().send(2)?;

        assert_eq!(receiver.recv().await?.content, 1);
        assert_eq!(receiver.recv().await?.content, 2);

        Ok(())
    }

    #[tokio::test]
    async fn sending_fails_once_the_receiver_is_dropped() {
        let (sender, receiver) = channel::<ReconciliationRequest>();

        drop(receiver);

        assert!(sender.send(ReconciliationRequest).is_err());
    }

    #[tokio::test]
    async fn receiving_fails_once_every_sender_is_dropped() {
        let (sender, receiver) = channel::<ReconciliationRequest>();

        drop(sender);

        assert!(receiver.recv().await.is_err());
    }
}
