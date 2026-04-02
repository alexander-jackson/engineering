output "address" {
  value       = aws_db_instance.this.address
  description = "The DNS hostname of the RDS instance endpoint"
}

output "security_group_id" {
  value       = aws_security_group.this.id
  description = "Security group ID attached to the RDS instance"
}
