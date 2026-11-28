resource "aws_iam_role" "this" {
  name        = format("%s-role", var.application_name)
  description = format("Role for the %s application container", var.application_name)

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          AWS = [for instance in var.instance_roles : instance.arn]
        }
      }
    ]
  })
}

resource "aws_iam_role_policy" "this" {
  name   = format("%s-policy", var.application_name)
  role   = aws_iam_role.this.name
  policy = var.policy_json
}

resource "aws_iam_role_policy" "instance_assume" {
  for_each = var.instance_roles

  name = format("%s-assume-role", var.application_name)
  role = each.value.name

  policy = jsonencode({
    Statement = [
      {
        Action   = "sts:AssumeRole"
        Effect   = "Allow"
        Resource = aws_iam_role.this.arn
      }
    ]
  })
}
