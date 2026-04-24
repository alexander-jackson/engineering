variable "db_password" {
  type        = string
  sensitive   = true
  description = "Master password for the RDS postgres instance"
}
