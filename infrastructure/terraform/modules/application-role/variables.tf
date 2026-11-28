variable "application_name" {
  type        = string
  description = "Name of the application container (used to name the role and inline policies)"
}

variable "instance_roles" {
  type = map(object({
    name = string
    arn  = string
  }))
  description = "EC2 instance IAM roles allowed to assume this role, keyed by a static label (e.g. the instance name)"
}

variable "policy_json" {
  type        = string
  description = "JSON-encoded IAM policy document to attach to the application role"
}
