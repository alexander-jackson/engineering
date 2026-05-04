variable "name" {
  type        = string
  description = "Unique identifier for the RDS instance"
}

variable "vpc_id" {
  type        = string
  description = "VPC in which to create the security group"
}

variable "subnet_ids" {
  type        = list(string)
  description = "Subnet IDs for the DB subnet group (must span at least 2 AZs)"
}

variable "availability_zone" {
  type        = string
  description = "Availability zone for the single-AZ instance placement"
}

variable "engine_version" {
  type        = string
  default     = "15"
  description = "PostgreSQL major version"
}

variable "instance_class" {
  type        = string
  default     = "db.t4g.micro"
  description = "RDS instance class"
}

variable "allocated_storage" {
  type        = number
  default     = 20
  description = "Storage allocation in GiB (minimum 20 for RDS)"
}

variable "password" {
  type        = string
  sensitive   = true
  description = "Master password for the postgres user"
}

variable "allow_major_version_upgrade" {
  type        = bool
  default     = false
  description = "Whether major version upgrades are allowed"
}

variable "apply_immediately" {
  type        = bool
  default     = true
  description = "Whether to apply changes immediately or during the next maintenance window"
}
