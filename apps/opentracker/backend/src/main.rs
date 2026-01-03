use std::env;
use std::net::{Ipv4Addr, SocketAddrV4};

use axum::Router;
use axum::routing::get;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgConnectOptions;
use sqlx::postgres::PgPool;
use tokio::net::TcpListener;

use opentracker::endpoints::{self, State};
use opentracker::trace_layer::TraceLayer;

/// Runs the setup for the server.
///
/// Sources the environment variables from `.env` and creates the logging instance.
fn setup() {
    // Populate the environment variables
    dotenvy::dotenv().ok();

    foundation_logging::install_default_registry();
}

async fn setup_database_pool() -> sqlx::Result<PgPool> {
    let host = env::var("DATABASE_HOST").expect("Failed to get a database host");
    let port = env::var("DATABASE_PORT")
        .map(|v| v.parse().expect("Invalid port"))
        .unwrap_or(5432);

    let database = env::var("DATABASE_NAME").expect("Failed to get a database name");

    let user = env::var("DATABASE_USER").ok();
    let password = env::var("DATABASE_PASSWORD").ok();

    let mut connect_options = PgConnectOptions::new()
        .host(&host)
        .port(port)
        .database(&database);

    if let Some(user) = user {
        connect_options = connect_options.username(&user);
    }

    if let Some(password) = password {
        connect_options = connect_options.password(&password);
    }

    let pool = PgPool::connect_with(connect_options).await?;

    Ok(pool)
}

async fn run_migrations(pool: &PgPool) -> sqlx::Result<()> {
    static MIGRATOR: Migrator = sqlx::migrate!();
    MIGRATOR.run(pool).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    setup();

    let pool = setup_database_pool()
        .await
        .expect("Failed to setup database pool");

    // Run the migrations
    run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    let state = State { pool };

    let app = Router::new()
        .route("/health", get(endpoints::health))
        .nest("/api", endpoints::router(state))
        .layer(TraceLayer);

    let host = Ipv4Addr::UNSPECIFIED;
    let port = 3025;
    let addr = SocketAddrV4::new(host, port);

    tracing::info!(?addr, "Listening for connections");

    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind listener");

    axum::serve(listener, app.into_make_service())
        .await
        .expect("Failed to serve application");
}
