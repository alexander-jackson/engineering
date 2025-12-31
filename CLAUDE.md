# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

This is a monorepo containing:
1. **Rust Applications**: Multiple Rust applications and shared foundation libraries in `apps/`
2. **Infrastructure**: Terraform configurations and service configurations in `infrastructure/`

Applications are containerized and deployed to AWS EC2 instances managed by Terraform. See `infrastructure/README.md` for detailed infrastructure documentation.

---

# Rust Applications

## Workspace Structure

This is a Cargo workspace (resolver v3) with applications in `apps/` and shared foundation libraries in `apps/foundation/`.

### Applications

All applications in `apps/`:

1. **dns-server** - DNS resolver with ad/tracker blocking, supports DNS-over-TLS
2. **pgmanager** - PostgreSQL backup management tool (S3 and filesystem storage)
3. **tag-updater** - Updates Docker image tags in infrastructure repositories via Git operations
4. **today** - Daily task/notes web application with PostgreSQL persistence
5. **uptime** - Uptime monitoring service with certificate expiry tracking and SNS alerts
6. **utils/application-bootstrap** - Utility for bootstrapping new applications

### Foundation Libraries

Shared libraries in `apps/foundation/`:

- **args** - CLI argument parsing (pico-args wrapper)
- **configuration** - YAML config loading with secret handling
- **database-bootstrap** - Database initialization utilities (sqlx integration)
- **http-server** - Axum-based HTTP server utilities
- **init** - Unified application initialization (args + config + logging + telemetry + database)
- **logging** - Tracing setup and configuration
- **recurring-job** - Framework for running background jobs
- **shutdown** - Graceful shutdown coordination for async tasks
- **telemetry** - OpenTelemetry integration for distributed tracing
- **uid** - Unique identifier generation and encoding

## Development Commands

### Building
```bash
# Build all workspace members
cargo build

# Build specific package
cargo build -p today
cargo build -p uptime
```

### Testing
```bash
# Run all tests
cargo test

# Run tests for specific package
cargo test -p foundation-init
```

### Running Applications
```bash
# Most applications use the foundation-init pattern
cargo run -p today -- --config path/to/config.yaml
cargo run -p uptime -- --config path/to/config.yaml
cargo run -p pgmanager -- --config path/to/config.yaml

# Some support additional flags (see --help)
cargo run -p pgmanager -- --help
```

## Architecture Patterns

### Application Initialization
Most applications use the `foundation-init` crate which provides a unified initialization pattern:
- Parses CLI arguments via `foundation-args`
- Loads YAML configuration via `foundation-configuration`
- Sets up logging via `foundation-logging`
- Optionally configures telemetry via `foundation-telemetry`
- Optionally bootstraps database via `foundation-database-bootstrap`

### Configuration Management
- Applications use YAML configuration files
- Secret values wrapped in `Secret<T>` type (prevents accidental logging)
- Configuration loaded via `ConfigurationReader::from_yaml()`
- In production: configs stored in S3 and pulled at instance startup

### Observability
- Structured logging with `tracing` crate
- Optional OpenTelemetry integration
- Logs collected by Vector and shipped to S3 in production
- Default logging registry provided by `foundation-logging`

### HTTP Services
- Built on Axum framework via `foundation-http-server`
- State management through Axum's `State` extractor
- Bearer token authentication patterns (see tag-updater)

### Database Integration
- Applications using PostgreSQL: `today`, `uptime`
- Connection via `database.mesh.internal` (private Route53 record)
- Database bootstrapping via `foundation-database-bootstrap`
- Migrations in `apps/{app}/migrations/`
- Backups automated via `pgmanager` service

### Background Jobs
- Use `foundation-recurring-job` for periodic tasks
- Examples: uptime polling (60s), certificate checking (daily)

### Error Handling
- Uses `color-eyre` for enhanced error reporting
- `Result<T>` return types throughout
- Context wrapping with `.wrap_err()` for detailed error messages

### Graceful Shutdown
- Use `foundation-shutdown` for coordinating shutdown of async tasks
- Ensures clean termination of background jobs, HTTP servers, etc.

## Common Dependencies

Workspace-level dependencies in root `Cargo.toml`:
- `axum = "0.8.4"` - HTTP framework
- `serde = "1.0.219"` - Serialization/deserialization
- `tracing = "0.1.41"` - Structured logging
- `sqlx` - PostgreSQL async driver (used by today, uptime)
- `tokio` - Async runtime

## Testing Strategy

- Unit tests in source files using `#[cfg(test)]`
- Integration tests in dedicated `tests/` directories
- Database tests use sqlx test macros

---

# Infrastructure

## Quick Reference

Detailed infrastructure documentation is in `infrastructure/README.md`. Key points:

### Directory Structure
- `infrastructure/terraform/` - Terraform IaC for AWS resources
- `infrastructure/configuration/` - Service configurations deployed to S3
- `infrastructure/certs/` - TLS certificates for development
- `infrastructure/scripts/` - Management scripts

### Deployment Architecture
Applications are containerized and deployed via an **f2 orchestrator**:
- F2 is an application load balancer running on EC2
- Handles TLS/mTLS termination
- Routes traffic to containerized services by domain/path
- Runs Docker containers for: frontend, backend, today, uptime, etc.

### Key Resources
- **EC2 Instances**: primary (f2 orchestrator), postgres, dns2
- **S3 Buckets**: configuration, logging, postgres-backups
- **Route53**: `opentracker.app`, `forkup.app`, private zone `mesh.internal`
- **PostgreSQL**: Dedicated instance, accessed via `database.mesh.internal`

### Terraform Commands
```bash
cd infrastructure/terraform

terraform init
terraform plan
terraform apply
terraform fmt -recursive
```

### Configuration Deployment
Service configs in `infrastructure/configuration/{service}/` are deployed to S3:
- Services: dns-server, f2, forkup, pgmanager, tag-updater, today, uptime, vector
- Deployed via GitHub workflow (deploy-config.yaml)
- Instances pull configs from S3 at startup

---

# CI/CD

GitHub workflows in `.github/workflows/`:
- **{app}-ci.yaml** - Run tests on PRs for each application
- **{app}-release.yaml** - Build and push Docker images on release tags
- **terraform-plan.yaml** - Plan infrastructure changes on PRs
- **terraform-apply.yaml** - Apply infrastructure changes on merge to master
- **deploy-config.yaml** - Deploy service configurations to S3

Release process:
1. Create git tag matching app name (e.g., `today-v1.2.3`)
2. Release workflow builds Docker image and pushes to registry
3. Tag-updater service automatically updates infrastructure repo with new image tag

---

# Database Migrations

Applications with SQL migrations:
- **today**: `apps/today/migrations/` (5 migrations)
- **uptime**: `apps/uptime/migrations/` (5 migrations)

Migrations are applied automatically at application startup via `foundation-database-bootstrap`.

---

# Key Development Workflows

## Adding a New Application

1. Use `application-bootstrap` utility or create new Cargo package in `apps/`
2. Add to workspace in root `Cargo.toml`
3. Use `foundation-init` for initialization if it's a service
4. Add configuration template to `infrastructure/configuration/{app}/`
5. Create CI/release workflows based on templates
6. Add Terraform resources if deploying to EC2

## Making Infrastructure Changes

1. Edit Terraform files in `infrastructure/terraform/`
2. Run `terraform plan` to preview changes
3. Create PR - terraform-plan workflow runs automatically
4. After merge, terraform-apply workflow applies changes

## Deploying Configuration Changes

1. Edit config in `infrastructure/configuration/{service}/`
2. Commit and push
3. deploy-config workflow uploads to S3
4. Restart service or wait for next deployment to pick up changes

## Working with Databases

- **Local development**: Use local PostgreSQL instance
- **Production**: Connect via `database.mesh.internal`
- **Backups**: Automated via pgmanager service (daily backups to S3)
- **Migrations**: Placed in `apps/{app}/migrations/`, auto-applied at startup
