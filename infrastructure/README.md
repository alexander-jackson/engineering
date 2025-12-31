# Infrastructure

Contains the infrastructure definitions for live projects.

## Directory Structure

- **terraform/** - Terraform IaC for AWS resources (VPC, EC2, S3, Route53, IAM)
- **configuration/** - Service configurations deployed to S3 (pulled by instances at startup)
- **certs/** - TLS certificates for development and mTLS authentication
- **scripts/** - Infrastructure management scripts

## Key Concepts

### F2 Orchestrator

The **f2** orchestrator is the core deployment pattern:
- Runs on the `primary` EC2 instance
- Acts as application load balancer and reverse proxy
- Manages Docker containers for all web services
- Handles TLS/mTLS termination and domain-based routing

Services are deployed as Docker containers managed by f2, not as standalone EC2 instances.

### Configuration Management

- Service configs stored in `configuration/{service}/`
- Deployed to S3 via `deploy-config.yaml` GitHub workflow
- Instances pull configs from S3 at startup using IAM instance profiles

### Internal Networking

- Private Route53 zone: `mesh.internal`
- `database.mesh.internal` → PostgreSQL instance
- Applications connect to shared PostgreSQL via internal DNS

### Logging

Vector runs on instances to collect Docker logs and ship to S3 logging bucket.

## Common Operations

### Deploy Infrastructure Changes

```bash
cd infrastructure/terraform
terraform plan
terraform apply
```

### Deploy Configuration Changes

Edit files in `configuration/{service}/`, commit and push. GitHub workflow deploys to S3 automatically.

### Certificate Management

```bash
# Generate new certificates
./generate-certificates.sh

# Renew existing certificates
./scripts/renew-certificates.sh
```

## Important Notes

- **Elastic IPs** used for all public-facing instances
- **User data ignored** in Terraform lifecycle to prevent instance replacement on script changes
- **PostgreSQL data** on separate EBS volume (`/dev/sdf`)
- **Remote state** stored in S3 with locking
