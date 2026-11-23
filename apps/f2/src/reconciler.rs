use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use color_eyre::eyre::Result;
use hyper::Uri;
use indexmap::IndexSet;
use tokio::sync::{Mutex, RwLock};

use crate::common::Container;
use crate::config::{Config, Diff, ExternalBytes, Probe, ReplicaCount, Service, ShutdownMode};
use crate::docker::api::{create_and_start_container, StartedContainerDetails};
use crate::docker::client::DockerClient;
use crate::docker::models::ContainerId;
use crate::health::HealthCheckResult;
use crate::ipc::{
    HealthCheckRequest, HealthCheckResponse, Receiver, ReconciliationRequest, Sender,
};
use crate::service_registry::ServiceRegistry;

/// A new version of a service that is waiting to prove itself healthy.
///
/// The candidate containers are not registered with the load balancer, and the containers
/// currently registered are left alone, until every candidate has reported as healthy.
#[derive(Debug)]
struct Rollout {
    definition: Service,
    /// Candidates which are yet to report back.
    pending: HashMap<ContainerId, StartedContainerDetails>,
    healthy: Vec<StartedContainerDetails>,
}

impl Rollout {
    fn candidates(&self) -> Vec<StartedContainerDetails> {
        self.pending
            .values()
            .cloned()
            .chain(self.healthy.iter().cloned())
            .collect()
    }
}

#[derive(Debug)]
pub struct Reconciler<C: DockerClient> {
    registry: Arc<RwLock<ServiceRegistry>>,
    config_location: Arc<ExternalBytes>,
    config: Arc<ArcSwap<Config>>,
    docker_client: C,
    reconciliations: Receiver<ReconciliationRequest>,
    health_checks: Sender<HealthCheckRequest>,
    health_responses: Receiver<HealthCheckResponse>,
    rollouts: Mutex<HashMap<String, Rollout>>,
}

impl<C: DockerClient> Reconciler<C> {
    pub fn new(
        registry: Arc<RwLock<ServiceRegistry>>,
        config_location: ExternalBytes,
        config: Arc<ArcSwap<Config>>,
        docker_client: C,
        reconciliations: Receiver<ReconciliationRequest>,
        health_checks: Sender<HealthCheckRequest>,
        health_responses: Receiver<HealthCheckResponse>,
    ) -> Self {
        Self {
            registry,
            config_location: Arc::new(config_location),
            config,
            docker_client,
            reconciliations,
            health_checks,
            health_responses,
            rollouts: Mutex::default(),
        }
    }

    pub async fn run(&self) -> Result<()> {
        loop {
            tokio::select! {
                request = self.reconciliations.recv() => {
                    if request.is_err() {
                        break;
                    }

                    tracing::info!("received signal to reconcile");
                    self.reconcile().await?;
                }
                response = self.health_responses.recv() => {
                    let Ok(response) = response else { break };

                    // A problem with one rollout shouldn't take the load balancer down
                    if let Err(error) = self.handle_health_response(response.content).await {
                        tracing::warn!(?error, "failed to handle a health check response");
                    }
                }
            }
        }

        Ok(())
    }

    async fn reconcile(&self) -> Result<()> {
        let new_config = Config::from_location(&self.config_location).await?;
        let old_config = self.config.load();

        if let Some(diff) = old_config.diff(&new_config) {
            self.config.store(Arc::new(new_config.clone()));

            for event in diff {
                self.handle_diff(event).await?;
            }

            self.docker_client.prune_images().await?;
        }

        Ok(())
    }

    async fn get_running_containers(
        &self,
        name: &str,
    ) -> Option<IndexSet<StartedContainerDetails>> {
        let read_lock = self.registry.read().await;

        read_lock.get_running_containers(name).cloned()
    }

    #[tracing::instrument(skip(self))]
    async fn start_multiple_containers(
        &self,
        name: &str,
        new_definition: Service,
        replicas: ReplicaCount,
    ) -> Result<()> {
        // Keep the locks short, create everything then add to the LB
        let mut started_containers = Vec::new();

        let private_key = self.config.load().get_private_key().await?;
        let container = Container::from(&new_definition);

        for _ in 0..replicas.get() {
            let details = create_and_start_container(
                &self.docker_client,
                &container,
                &new_definition.tag,
                private_key.as_ref(),
            )
            .await?;

            started_containers.push(details);
        }

        let probe = new_definition.liveness_probe().cloned();

        let mut write_lock = self.registry.write().await;
        write_lock.define(name, new_definition);

        for details in &started_containers {
            write_lock.add_container(name, details.clone());
        }

        drop(write_lock);

        let Some(probe) = probe else {
            tracing::debug!(%name, "no liveness probe defined, skipping health checks");
            return Ok(());
        };

        for details in started_containers {
            self.request_health_check(name, details, &probe);
        }

        Ok(())
    }

    /// Requests a health check, returning whether the request was made.
    fn request_health_check(
        &self,
        name: &str,
        details: StartedContainerDetails,
        probe: &Probe,
    ) -> bool {
        let target = match health_check_target(details.addr, probe) {
            Ok(target) => target,
            Err(error) => {
                tracing::warn!(%name, ?error, "failed to build health check target");
                return false;
            }
        };

        let request = HealthCheckRequest {
            service: name.to_owned(),
            container_id: details.id,
            target,
        };

        if let Err(error) = self.health_checks.send(request) {
            tracing::warn!(%name, ?error, "failed to request a health check");
            return false;
        }

        true
    }

    /// Removes containers that never made it into the registry, logging rather than failing.
    async fn discard_containers(&self, containers: &[StartedContainerDetails]) {
        for details in containers {
            if let Err(error) = self.docker_client.remove_container(&details.id).await {
                tracing::warn!(container_id = %details.id, ?error, "failed to remove container");
            }
        }
    }

    /// Abandons any rollout in progress for the service, removing its candidate containers.
    async fn cancel_rollout(&self, name: &str) {
        let rollout = self.rollouts.lock().await.remove(name);

        if let Some(rollout) = rollout {
            tracing::info!(%name, "cancelling rollout");
            self.discard_containers(&rollout.candidates()).await;
        }
    }

    /// Starts the candidate containers for a new definition and requests their health checks.
    ///
    /// This supersedes any rollout already in progress. Nothing is retried on failure, the
    /// previous version stays in place until a new definition is deployed.
    async fn start_rollout(&self, name: &str, definition: Service) -> Result<()> {
        self.cancel_rollout(name).await;

        let mut started = Vec::new();

        if let Err(error) = self.start_candidates(&definition, &mut started).await {
            tracing::warn!(%name, ?error, "failed to start containers for rollout, the previous version is kept");
            self.discard_containers(&started).await;

            return Ok(());
        }

        let Some(probe) = definition.liveness_probe().cloned() else {
            return Ok(());
        };

        let rollout = Rollout {
            definition,
            pending: started
                .iter()
                .map(|details| (details.id.clone(), details.clone()))
                .collect(),
            healthy: Vec::new(),
        };

        self.rollouts.lock().await.insert(name.to_owned(), rollout);

        for details in started {
            if !self.request_health_check(name, details, &probe) {
                tracing::warn!(%name, "failed to request health checks for rollout, the previous version is kept");
                self.cancel_rollout(name).await;

                break;
            }
        }

        Ok(())
    }

    async fn start_candidates(
        &self,
        definition: &Service,
        started: &mut Vec<StartedContainerDetails>,
    ) -> Result<()> {
        let private_key = self.config.load().get_private_key().await?;
        let container = Container::from(definition);

        for _ in 0..definition.replicas.get() {
            let details = create_and_start_container(
                &self.docker_client,
                &container,
                &definition.tag,
                private_key.as_ref(),
            )
            .await?;

            started.push(details);
        }

        Ok(())
    }

    async fn handle_health_response(&self, response: HealthCheckResponse) -> Result<()> {
        let HealthCheckResponse {
            service,
            container_id,
            result,
        } = response;

        let mut rollouts = self.rollouts.lock().await;

        let Some(rollout) = rollouts.get_mut(&service) else {
            return Ok(());
        };

        // Responses for unknown containers belong to a rollout that was cancelled
        let Some(details) = rollout.pending.get(&container_id).cloned() else {
            return Ok(());
        };

        match result {
            HealthCheckResult::Success => {
                rollout.pending.remove(&container_id);
                rollout.healthy.push(details);

                if rollout.pending.is_empty() {
                    drop(rollouts);

                    return self.complete_rollout(&service).await;
                }

                Ok(())
            }
            HealthCheckResult::Failure => {
                drop(rollouts);

                tracing::warn!(%service, %container_id, "container failed its health check, the previous version is kept");
                self.cancel_rollout(&service).await;

                Ok(())
            }
        }
    }

    /// Routes traffic to the healthy containers and away from the previous version.
    async fn complete_rollout(&self, name: &str) -> Result<()> {
        let Some(rollout) = self.rollouts.lock().await.remove(name) else {
            return Ok(());
        };

        tracing::info!(%name, "rollout is healthy");

        let mut write_lock = self.registry.write().await;

        let previous = write_lock
            .get_running_containers(name)
            .cloned()
            .unwrap_or_default();
        let shutdown_mode = write_lock
            .get_definition(name)
            .map(|definition| definition.shutdown_mode)
            .unwrap_or_default();

        write_lock.define(name, rollout.definition);

        for details in rollout.healthy {
            write_lock.add_container(name, details);
        }

        for details in &previous {
            write_lock.remove_container_by_id(name, &details.id);
        }

        drop(write_lock);

        self.shut_down_containers(&previous, shutdown_mode).await?;
        self.docker_client.prune_images().await
    }

    async fn shut_down_containers(
        &self,
        containers: &IndexSet<StartedContainerDetails>,
        shutdown_mode: ShutdownMode,
    ) -> Result<()> {
        for details in containers {
            match shutdown_mode {
                ShutdownMode::Graceful => {
                    self.docker_client.stop_container(&details.id).await?;
                    self.docker_client.remove_container(&details.id).await?;
                }
                ShutdownMode::Forceful => {
                    self.docker_client.remove_container(&details.id).await?;
                }
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn handle_alteration(
        &self,
        name: &str,
        old_definition: Service,
        new_definition: Service,
    ) -> Result<()> {
        if new_definition.requires_healthy_containers() {
            return self.start_rollout(name, new_definition).await;
        }

        // Anything still being rolled out is superseded by this definition
        self.cancel_rollout(name).await;

        // There may be none if a rollout that was cancelled above never became healthy
        let running_containers = self.get_running_containers(name).await.unwrap_or_default();

        let replicas = new_definition.replicas;

        self.start_multiple_containers(name, new_definition, replicas)
            .await?;

        let mut write_lock = self.registry.write().await;

        for details in &running_containers {
            write_lock.remove_container_by_id(name, &details.id);
        }

        drop(write_lock);

        self.shut_down_containers(&running_containers, old_definition.shutdown_mode)
            .await
    }

    #[tracing::instrument(skip(self))]
    async fn handle_addition(&self, name: String, definition: Service) -> Result<()> {
        if definition.requires_healthy_containers() {
            return self.start_rollout(&name, definition).await;
        }

        let replicas = definition.replicas;

        self.start_multiple_containers(&name, definition, replicas)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn handle_removal(&self, name: String) -> Result<()> {
        self.cancel_rollout(&name).await;

        let running_containers = self.get_running_containers(&name).await;

        // Remove them from the LB
        if let Some(containers) = running_containers {
            let mut write_lock = self.registry.write().await;

            write_lock.undefine(&name);
            write_lock.remove_all_containers(&name);

            drop(write_lock);

            for details in &containers {
                self.docker_client.remove_container(&details.id).await?;
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn handle_diff(&self, diff: Diff) -> Result<()> {
        match diff {
            Diff::Alteration {
                name,
                old_definition,
                new_definition,
            } => {
                self.handle_alteration(&name, old_definition, new_definition)
                    .await?
            }
            Diff::Addition { name, definition } => self.handle_addition(name, definition).await?,
            Diff::Removal { name } => self.handle_removal(name).await?,
        }

        Ok(())
    }
}

fn health_check_target(addr: Ipv4Addr, probe: &Probe) -> Result<Uri> {
    let Probe { path, port, .. } = probe;
    let separator = if path.starts_with('/') { "" } else { "/" };

    Ok(format!("http://{addr}:{port}{separator}{path}").parse()?)
}

#[cfg(test)]
pub mod tests {
    use std::collections::{HashMap, HashSet};
    use std::net::Ipv4Addr;
    use std::path::PathBuf;
    use std::sync::Arc;

    use arc_swap::ArcSwap;
    use color_eyre::eyre::Result;
    use tokio::sync::RwLock;

    use crate::common::Environment;
    use crate::config::{
        AlbConfig, Config, Diff, ExternalBytes, Probe, Probes, ReplicaCount, Scheme, Service,
    };
    use crate::docker::api::StartedContainerDetails;
    use crate::docker::client::DockerClient;
    use crate::docker::models::{ContainerId, ImageSummary, NetworkId};
    use crate::health::HealthCheckResult;
    use crate::ipc::{channel, HealthCheckRequest, HealthCheckResponse, Receiver};
    use crate::reconciler::{health_check_target, Reconciler};
    use crate::service_registry::ServiceRegistry;

    #[derive(Clone, Debug, Default)]
    struct DockerState {
        images: Vec<ImageSummary>,
        containers: Vec<(ContainerId, String)>,
    }

    #[derive(Clone, Default)]
    pub struct FakeDockerClient {
        state: Arc<RwLock<DockerState>>,
    }

    #[async_trait::async_trait]
    impl DockerClient for FakeDockerClient {
        async fn fetch_images(&self) -> Result<Vec<ImageSummary>> {
            let lock = self.state.read().await;

            Ok(lock.images.clone())
        }

        async fn pull_image(&self, _image: &str, _tag: &str) -> Result<()> {
            Ok(())
        }

        async fn create_container(
            &self,
            image: &str,
            _environment: &Option<Environment>,
            _docker_volumes: &HashMap<String, String>,
            _network: Option<(&NetworkId, &str)>,
            _args: &[String],
        ) -> Result<ContainerId> {
            let container_id = ContainerId::random();

            let mut lock = self.state.write().await;
            lock.containers
                .push((container_id.clone(), image.to_owned()));

            Ok(container_id)
        }

        async fn start_container(&self, _id: &ContainerId) -> Result<()> {
            Ok(())
        }

        async fn get_container_ip(&self, _id: &ContainerId) -> Result<Ipv4Addr> {
            Ok(Ipv4Addr::LOCALHOST)
        }

        async fn stop_container(&self, id: &ContainerId) -> Result<()> {
            let mut lock = self.state.write().await;
            lock.containers.retain(|c| c.0 != *id);

            Ok(())
        }

        async fn remove_container(&self, id: &ContainerId) -> Result<()> {
            let mut lock = self.state.write().await;
            lock.containers.retain(|c| c.0 != *id);

            Ok(())
        }

        async fn get_network_by_name(&self, _name: &str) -> Result<Option<NetworkId>> {
            Ok(Some(NetworkId("mesh".to_owned())))
        }

        async fn prune_images(&self) -> Result<()> {
            let mut lock = self.state.write().await;
            let in_use: HashSet<String> =
                lock.containers.iter().map(|(_, img)| img.clone()).collect();
            lock.images
                .retain(|img| img.repo_tags.iter().any(|t| in_use.contains(t)));
            Ok(())
        }
    }

    fn create_reconciler<C: DockerClient>(
        registry: ServiceRegistry,
        docker_client: C,
    ) -> (Reconciler<C>, Receiver<HealthCheckRequest>) {
        let config = Config {
            alb: AlbConfig {
                addr: Ipv4Addr::LOCALHOST,
                ports: HashMap::from([(Scheme::Http, 5000)]),
                reconciliation: String::new(),
                tls: None,
                mtls: None,
            },
            secrets: None,
            services: HashMap::new(),
        };

        let config = ArcSwap::from_pointee(config);

        let (_, reconciliations) = channel();
        let (health_checks, requests) = channel();
        let (_, health_responses) = channel();

        let reconciler = Reconciler::new(
            Arc::new(RwLock::new(registry)),
            ExternalBytes::Filesystem {
                path: PathBuf::new(),
            },
            Arc::new(config),
            docker_client,
            reconciliations,
            health_checks,
            health_responses,
        );

        (reconciler, requests)
    }

    #[tokio::test]
    async fn can_handle_addition_of_services() -> Result<()> {
        let registry = ServiceRegistry::new();

        let image = "alexanderjackson/f2";
        let tag = "latest";

        let service = Service {
            image: image.to_owned(),
            tag: tag.to_owned(),
            ..Default::default()
        };

        let docker_client = FakeDockerClient::default();
        let (reconciler, _checks) = create_reconciler(registry, docker_client.clone());

        reconciler
            .handle_diff(Diff::Addition {
                name: "foobar".to_owned(),
                definition: service,
            })
            .await?;

        // Check we now have some containers in the Docker state
        let lock = docker_client.state.read().await;
        let containers = &lock.containers;

        let container_id = containers.iter().find_map(|(id, image_and_tag)| {
            image_and_tag.eq(&format!("{image}:{tag}")).then_some(id)
        });

        assert!(container_id.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn can_handle_removal_of_service() -> Result<()> {
        let mut registry = ServiceRegistry::new();

        let service = "foobar";
        let image = "alexanderjackson/f2";
        let tag = "latest";

        let docker_client = FakeDockerClient::default();

        let id = docker_client
            .create_container(
                &format!("{image}:{tag}"),
                &None,
                &HashMap::new(),
                Some((&NetworkId("mesh".to_owned()), "foobar.local")),
                &[],
            )
            .await?;

        registry.add_container(
            service,
            StartedContainerDetails {
                id,
                addr: Ipv4Addr::LOCALHOST,
            },
        );

        let (reconciler, _checks) = create_reconciler(registry, docker_client.clone());

        let diff = Diff::Removal {
            name: service.to_owned(),
        };

        reconciler.handle_diff(diff).await?;

        // Check the state is now empty
        let lock = docker_client.state.read().await;
        let containers = &lock.containers;

        assert!(containers.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn can_handle_scale_up_of_service() -> Result<()> {
        let mut registry = ServiceRegistry::new();

        let service = "foobar";
        let image = "alexanderjackson/f2";
        let tag = "latest";

        let docker_client = FakeDockerClient::default();

        let service_definition = Service {
            image: image.to_owned(),
            tag: tag.to_owned(),
            ..Default::default()
        };

        let mut altered_definition = service_definition.clone();
        altered_definition.replicas = ReplicaCount::try_from(2)?;

        let image_and_tag = format!("{image}:{tag}");
        let id = docker_client
            .create_container(
                &image_and_tag,
                &None,
                &HashMap::new(),
                Some((&NetworkId("mesh".to_owned()), "foobar.local")),
                &[],
            )
            .await?;

        registry.define(service, service_definition.clone());
        registry.add_container(
            service,
            StartedContainerDetails {
                id: id.clone(),
                addr: Ipv4Addr::LOCALHOST,
            },
        );

        let (reconciler, _checks) = create_reconciler(registry, docker_client.clone());

        let diff = Diff::Alteration {
            name: service.to_owned(),
            old_definition: service_definition,
            new_definition: altered_definition,
        };

        reconciler.handle_diff(diff).await?;

        // Check we now have 2 containers for this image and tag
        let lock = docker_client.state.read().await;
        let containers = &lock.containers;
        let matching_containers = containers.iter().filter(|c| c.1 == image_and_tag).count();

        assert_eq!(matching_containers, 2);

        // Neither of these containers are our original one
        let original_container = containers.iter().find(|c| c.0 == id);
        assert!(original_container.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn prunes_unused_images_after_tag_change() -> Result<()> {
        let mut registry = ServiceRegistry::new();

        let service = "foobar";
        let image = "myapp";
        let old_tag = "v1";
        let new_tag = "v2";

        let docker_client = FakeDockerClient::default();

        // Pre-seed the old image on the host
        {
            let mut lock = docker_client.state.write().await;
            lock.images.push(ImageSummary {
                repo_tags: vec![format!("{image}:{old_tag}")],
            });
        }

        // Create a running container for v1
        let id = docker_client
            .create_container(
                &format!("{image}:{old_tag}"),
                &None,
                &HashMap::new(),
                Some((&NetworkId("mesh".to_owned()), "foobar.local")),
                &[],
            )
            .await?;

        let old_definition = Service {
            image: image.to_owned(),
            tag: old_tag.to_owned(),
            ..Default::default()
        };
        let new_definition = Service {
            image: image.to_owned(),
            tag: new_tag.to_owned(),
            ..Default::default()
        };

        registry.define(service, old_definition.clone());
        registry.add_container(
            service,
            StartedContainerDetails {
                id,
                addr: Ipv4Addr::LOCALHOST,
            },
        );

        let (reconciler, _checks) = create_reconciler(registry, docker_client.clone());

        reconciler
            .handle_diff(Diff::Alteration {
                name: service.to_owned(),
                old_definition,
                new_definition,
            })
            .await?;

        docker_client.prune_images().await?;

        let lock = docker_client.state.read().await;

        // Old image should be gone
        let old_image_present = lock
            .images
            .iter()
            .any(|img| img.repo_tags.contains(&format!("{image}:{old_tag}")));
        assert!(!old_image_present, "old image should have been pruned");

        // New container should exist
        let new_container_present = lock
            .containers
            .iter()
            .any(|(_, img)| img == &format!("{image}:{new_tag}"));
        assert!(new_container_present, "new container should be running");

        Ok(())
    }

    fn probes(path: &str, port: u16) -> Option<Probes> {
        Some(Probes {
            liveness: Some(Probe {
                path: path.to_owned(),
                port,
                required: false,
            }),
        })
    }

    #[test]
    fn health_check_target_normalises_the_path() -> Result<()> {
        let probe = |path: &str| Probe {
            path: path.to_owned(),
            port: 8000,
            required: false,
        };

        let expected = "http://127.0.0.1:8000/health";
        assert_eq!(
            health_check_target(Ipv4Addr::LOCALHOST, &probe("/health"))?,
            expected
        );
        assert_eq!(
            health_check_target(Ipv4Addr::LOCALHOST, &probe("health"))?,
            expected
        );

        Ok(())
    }

    #[tokio::test]
    async fn requests_health_check_for_started_containers() -> Result<()> {
        let docker_client = FakeDockerClient::default();
        let (reconciler, checks) = create_reconciler(ServiceRegistry::new(), docker_client);

        let service = Service {
            image: "alexanderjackson/f2".to_owned(),
            tag: "latest".to_owned(),
            probes: probes("/health", 8000),
            ..Default::default()
        };

        reconciler
            .handle_diff(Diff::Addition {
                name: "foobar".to_owned(),
                definition: service,
            })
            .await?;

        let received = checks.recv().await?;

        assert_eq!(received.content.service, "foobar");
        assert_eq!(received.content.target, "http://127.0.0.1:8000/health");

        Ok(())
    }

    #[tokio::test]
    async fn does_not_request_health_check_without_probes() -> Result<()> {
        let docker_client = FakeDockerClient::default();
        let (reconciler, checks) = create_reconciler(ServiceRegistry::new(), docker_client);

        let service = Service {
            image: "alexanderjackson/f2".to_owned(),
            tag: "latest".to_owned(),
            ..Default::default()
        };

        reconciler
            .handle_diff(Diff::Addition {
                name: "foobar".to_owned(),
                definition: service,
            })
            .await?;

        let received =
            tokio::time::timeout(std::time::Duration::from_millis(10), checks.recv()).await;

        assert!(received.is_err());

        Ok(())
    }

    fn required_service(tag: &str) -> Service {
        let mut probes = probes("/health", 8000);
        probes.as_mut().unwrap().liveness.as_mut().unwrap().required = true;

        Service {
            image: "myapp".to_owned(),
            tag: tag.to_owned(),
            probes,
            ..Default::default()
        }
    }

    async fn registered<C: DockerClient>(
        reconciler: &Reconciler<C>,
        name: &str,
    ) -> Vec<ContainerId> {
        reconciler
            .get_running_containers(name)
            .await
            .map(|containers| containers.iter().map(|c| c.id.clone()).collect())
            .unwrap_or_default()
    }

    async fn docker_containers(client: &FakeDockerClient) -> Vec<ContainerId> {
        let lock = client.state.read().await;

        lock.containers.iter().map(|(id, _)| id.clone()).collect()
    }

    async fn respond(
        reconciler: &Reconciler<FakeDockerClient>,
        service: &str,
        container_id: ContainerId,
        result: HealthCheckResult,
    ) -> Result<()> {
        reconciler
            .handle_health_response(HealthCheckResponse {
                service: service.to_owned(),
                container_id,
                result,
            })
            .await
    }

    /// Takes the container the reconciler has asked to be health checked next.
    async fn next_checked(checks: &Receiver<HealthCheckRequest>) -> Result<ContainerId> {
        let request = checks.recv().await?;

        Ok(request.content.container_id)
    }

    /// An existing, registered, container running the `v1` tag of `myapp`.
    async fn registry_with_running_service(
        client: &FakeDockerClient,
        name: &str,
    ) -> Result<(ServiceRegistry, ContainerId)> {
        let id = client
            .create_container("myapp:v1", &None, &HashMap::new(), None, &[])
            .await?;

        let mut registry = ServiceRegistry::new();
        registry.define(name, required_service("v1"));
        registry.add_container(
            name,
            StartedContainerDetails {
                id: id.clone(),
                addr: Ipv4Addr::LOCALHOST,
            },
        );

        Ok((registry, id))
    }

    #[tokio::test]
    async fn required_additions_are_only_routed_once_healthy() -> Result<()> {
        let docker_client = FakeDockerClient::default();
        let (reconciler, checks) = create_reconciler(ServiceRegistry::new(), docker_client.clone());

        reconciler
            .handle_addition("foobar".to_owned(), required_service("v1"))
            .await?;

        let candidate = next_checked(&checks).await?;

        assert!(registered(&reconciler, "foobar").await.is_empty());
        assert_eq!(
            docker_containers(&docker_client).await,
            vec![candidate.clone()]
        );

        respond(
            &reconciler,
            "foobar",
            candidate.clone(),
            HealthCheckResult::Success,
        )
        .await?;

        assert_eq!(registered(&reconciler, "foobar").await, vec![candidate]);

        Ok(())
    }

    #[tokio::test]
    async fn previous_version_is_replaced_once_the_new_one_is_healthy() -> Result<()> {
        let docker_client = FakeDockerClient::default();
        let (registry, old) = registry_with_running_service(&docker_client, "foobar").await?;
        let (reconciler, checks) = create_reconciler(registry, docker_client.clone());

        reconciler
            .handle_alteration("foobar", required_service("v1"), required_service("v2"))
            .await?;

        let candidate = next_checked(&checks).await?;

        // Nothing has moved until the candidate reports back
        assert_eq!(registered(&reconciler, "foobar").await, vec![old.clone()]);
        assert!(docker_containers(&docker_client).await.contains(&old));

        respond(
            &reconciler,
            "foobar",
            candidate.clone(),
            HealthCheckResult::Success,
        )
        .await?;

        assert_eq!(
            registered(&reconciler, "foobar").await,
            vec![candidate.clone()]
        );
        assert_eq!(docker_containers(&docker_client).await, vec![candidate]);

        Ok(())
    }

    #[tokio::test]
    async fn unhealthy_rollouts_keep_the_previous_version() -> Result<()> {
        let docker_client = FakeDockerClient::default();
        let (registry, old) = registry_with_running_service(&docker_client, "foobar").await?;
        let (reconciler, checks) = create_reconciler(registry, docker_client.clone());

        reconciler
            .handle_alteration("foobar", required_service("v1"), required_service("v2"))
            .await?;

        let first = next_checked(&checks).await?;
        respond(
            &reconciler,
            "foobar",
            first.clone(),
            HealthCheckResult::Failure,
        )
        .await?;

        // The candidate is gone, the previous version is still serving
        assert_eq!(registered(&reconciler, "foobar").await, vec![old.clone()]);
        assert_eq!(docker_containers(&docker_client).await, vec![old.clone()]);

        // Nothing is retried
        assert!(reconciler.rollouts.lock().await.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn newer_definitions_supersede_a_rollout_in_progress() -> Result<()> {
        let docker_client = FakeDockerClient::default();
        let (registry, old) = registry_with_running_service(&docker_client, "foobar").await?;
        let (reconciler, checks) = create_reconciler(registry, docker_client.clone());

        reconciler
            .handle_alteration("foobar", required_service("v1"), required_service("v2"))
            .await?;
        let superseded = next_checked(&checks).await?;

        reconciler
            .handle_alteration("foobar", required_service("v2"), required_service("v3"))
            .await?;
        let current = next_checked(&checks).await?;

        // The first candidate was removed, and its late response changes nothing
        assert!(!docker_containers(&docker_client)
            .await
            .contains(&superseded));
        respond(
            &reconciler,
            "foobar",
            superseded,
            HealthCheckResult::Success,
        )
        .await?;
        assert_eq!(registered(&reconciler, "foobar").await, vec![old]);

        respond(
            &reconciler,
            "foobar",
            current.clone(),
            HealthCheckResult::Success,
        )
        .await?;
        assert_eq!(registered(&reconciler, "foobar").await, vec![current]);

        Ok(())
    }
}
