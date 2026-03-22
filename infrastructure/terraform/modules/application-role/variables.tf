variable "application_name" {
  type        = string
  description = "Name of the application container (used to name the role and inline policies)"
}

variable "instance_role_name" {
  type        = string
  description = "Name of the EC2 instance IAM role that will be allowed to assume this role"
}

variable "policy_json" {
  type        = string
  description = "JSON-encoded IAM policy document to attach to the application role"
}
