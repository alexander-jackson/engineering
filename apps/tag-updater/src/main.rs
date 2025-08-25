use std::net::SocketAddrV4;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::routing::put;
use axum::{Json, Router};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use color_eyre::eyre::{Context, Result, eyre};
use foundation_configuration::{ConfigurationReader, Secret};
use git2::Repository;
use serde::Deserialize;
use tokio::net::TcpListener;

mod args;
mod config;
mod editor;
mod git;

use crate::args::Args;
use crate::config::Config;
use crate::editor::make_tag_edit;

#[derive(Clone)]
struct SharedState {
    passphrase: Arc<Secret<String>>,
    ssh_private_key: Arc<Secret<String>>,
    repository: Arc<Mutex<Repository>>,
}

fn setup() -> Result<()> {
    color_eyre::install()?;
    foundation_logging::install_default_registry()?;

    Ok(())
}

fn get_repository_or_clone(
    filepath: &Path,
    url: &str,
    private_key: &str,
) -> Result<Arc<Mutex<Repository>>> {
    let repository = if filepath.exists() {
        Repository::open(filepath)
            .wrap_err_with(|| eyre!("Failed to open repository at {filepath:?}"))?
    } else {
        git::clone(url, filepath, private_key)?
    };

    Ok(Arc::new(Mutex::new(repository)))
}

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    let args = Args::from_env()?;
    let config = Config::from_yaml(&args.config)?;

    let private_key_bytes = config.private_key.resolve().await?;
    let private_key = String::from_utf8(private_key_bytes)
        .wrap_err("Failed to parse private key as UTF-8 string")?;

    let repository = get_repository_or_clone(
        Path::new("/tmp/infrastructure"),
        "git@github.com:alexander-jackson/infrastructure.git",
        &private_key,
    )?;

    tracing::info!("Successfully opened a repository for processing");

    let shared_state = SharedState {
        passphrase: Arc::from(config.passphrase),
        ssh_private_key: Arc::from(Secret::from(private_key)),
        repository,
    };

    let router = Router::new()
        .route("/update", put(handle_tag_update))
        .with_state(shared_state);

    let addr = SocketAddrV4::new(config.addr, config.port);
    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, router.into_make_service()).await?;

    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
struct TagUpdate {
    service: String,
    tag: String,
}

async fn handle_tag_update(
    State(state): State<SharedState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Json(update): Json<TagUpdate>,
) {
    let token = authorization.token();

    // Check the request state
    if token != **state.passphrase {
        return tracing::warn!(
            "Invalid request, token was {token} which did not match the passphrase"
        );
    }

    let TagUpdate { service, tag } = &update;

    let repository = state.repository.lock().unwrap();
    let path = repository.path();

    tracing::info!("Fetching latest changes for {path:?} to handle {update:?}");

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

    make_tag_edit(&root.join(&config), service, tag).unwrap();

    // Add the file to the index and write it to disk
    let mut index = git::add(&repository, &config).unwrap();

    // Make a new commit
    let commit_oid = git::commit(&repository, &mut index, service).unwrap();

    tracing::info!("Created a new commit {commit_oid} with the changes");

    let remote_name = "origin";
    let remote_ref = "refs/heads/master";

    git::push(&repository, remote_name, remote_ref, &state.ssh_private_key).unwrap();

    tracing::info!("Successfully pushed changes up to the remote ({remote_name})");
}
