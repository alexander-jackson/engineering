use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::put;
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use color_eyre::eyre::{Context, Result, eyre};
use foundation_configuration::Secret;
use foundation_http_server::Server;
use foundation_init::Configuration;
use git2::Repository;
use serde::Deserialize;
use tokio::net::TcpListener;

mod config;
mod editor;
mod git;

use crate::config::ApplicationConfiguration;
use crate::editor::make_tag_edit;

#[derive(Clone)]
struct SharedState {
    passphrase: Arc<Secret<String>>,
    ssh_private_key: Arc<Secret<String>>,
    repository: Arc<Mutex<Repository>>,
    repository_path: PathBuf,
    repository_url: String,
}

#[tracing::instrument]
fn get_repository_or_clone(
    filepath: &Path,
    url: &str,
    private_key: &Secret<String>,
) -> Result<Repository> {
    let repository = if filepath.exists() {
        Repository::open(filepath)
            .wrap_err_with(|| eyre!("Failed to open repository at {filepath:?}"))?
    } else {
        git::clone(url, filepath, private_key)?
    };

    Ok(repository)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config: Configuration<ApplicationConfiguration> = foundation_init::run()?;

    let private_key_bytes = config.private_key.resolve().await?;
    let private_key = String::from_utf8(private_key_bytes)
        .wrap_err("Failed to parse private key as UTF-8 string")?;

    let ssh_private_key = Arc::from(Secret::from(private_key));
    let repository_path = Path::new("/tmp/infrastructure");
    let repository_url = "git@github.com:alexander-jackson/infrastructure.git";

    let repository = get_repository_or_clone(repository_path, repository_url, &ssh_private_key)?;

    tracing::info!("successfully opened a repository for processing");

    let shared_state = SharedState {
        passphrase: Arc::from(config.passphrase.clone()),
        ssh_private_key,
        repository: Arc::new(Mutex::new(repository)),
        repository_path: repository_path.to_owned(),
        repository_url: repository_url.to_string(),
    };

    let server = Server::new()
        .route("/update", put(handle_tag_update))
        .with_state(shared_state);

    let addr = SocketAddrV4::new(config.addr, config.port);
    let listener = TcpListener::bind(addr).await?;

    server.run(listener).await?;

    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
struct TagUpdate {
    service: String,
    tag: String,
}

#[tracing::instrument(skip(state, authorization))]
async fn handle_tag_update(
    State(state): State<SharedState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Json(update): Json<TagUpdate>,
) -> StatusCode {
    let token = authorization.token();

    // Check the request state
    if token != **state.passphrase {
        tracing::warn!("Invalid request, token was {token} which did not match the passphrase");

        return StatusCode::UNAUTHORIZED;
    }

    let TagUpdate { service, tag } = &update;

    let repository = match state.repository.lock() {
        Ok(repo) => repo,
        Err(err) => {
            tracing::warn!(
                ?err,
                "failed to acquire repository lock, attempting to recover"
            );

            // remove the existing repository and re-clone it as we don't know the state
            std::fs::remove_dir_all(&state.repository_path).unwrap();

            *state.repository.lock().unwrap() = get_repository_or_clone(
                &state.repository_path,
                &state.repository_url,
                &state.ssh_private_key,
            )
            .unwrap();
            state.repository.clear_poison();

            state.repository.lock().unwrap()
        }
    };

    let path = repository.path();

    tracing::info!(?path, "fetching the latest changes");

    let mut remote = repository.find_remote("origin").unwrap();

    let latest = git::fetch(
        &repository,
        &["master"],
        &mut remote,
        &state.ssh_private_key,
    )
    .unwrap();

    git::merge(&repository, "master", &latest).unwrap();

    // Make the tag update
    let root = path.parent().unwrap();
    let config = Path::new("configuration").join("f2").join("config.yaml");

    if let Err(e) = make_tag_edit(&root.join(&config), service, tag) {
        tracing::error!(?e, "failed to make the tag edit");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Add the file to the index and write it to disk
    let mut index = git::add(&repository, &config).unwrap();

    // Make a new commit
    let commit_oid = git::commit(&repository, &mut index, service).unwrap();

    tracing::info!(oid = %commit_oid, "created a new commit with the changes");

    let remote_name = "origin";
    let remote_ref = "refs/heads/master";

    git::push(&repository, remote_name, remote_ref, &state.ssh_private_key).unwrap();

    tracing::info!(%remote_name, %remote_ref, "pushed the changes to the remote");

    StatusCode::OK
}
