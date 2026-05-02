resource "aws_security_group" "this" {
  name        = format("%s-rds-postgres", var.name)
  description = format("Security group for the %s RDS instance", var.name)
  vpc_id      = var.vpc_id
}

resource "aws_security_group_rule" "allow_inbound_postgres" {
  for_each = toset(var.allowed_security_group_ids)

  description              = format("Allow inbound PostgreSQL from %s", each.key)
  type                     = "ingress"
  from_port                = 5432
  to_port                  = 5432
  protocol                 = "tcp"
  source_security_group_id = each.key
  security_group_id        = aws_security_group.this.id
}

resource "aws_db_subnet_group" "this" {
  name       = var.name
  subnet_ids = var.subnet_ids
}

resource "aws_db_instance" "this" {
  identifier     = var.name
  engine         = "postgres"
  engine_version = var.engine_version
  instance_class = var.instance_class

  allocated_storage = var.allocated_storage
  storage_type      = "gp2"

  db_name                     = "postgres"
  username                    = "postgres"
  password = var.password

  db_subnet_group_name   = aws_db_subnet_group.this.name
  vpc_security_group_ids = [aws_security_group.this.id]

  multi_az            = false
  availability_zone   = var.availability_zone
  publicly_accessible = false

  backup_retention_period   = 1
  skip_final_snapshot       = false
  final_snapshot_identifier = format("%s-final-snapshot", var.name)

  allow_major_version_upgrade = var.allow_major_version_upgrade
}
