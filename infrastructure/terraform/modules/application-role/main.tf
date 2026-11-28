data "aws_iam_role" "instance" {
  for_each = var.instance_role_names

  name = each.value
}

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
          AWS = [for role in data.aws_iam_role.instance : role.arn]
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
  for_each = var.instance_role_names

  name = format("%s-assume-role", var.application_name)
  role = each.value

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

# Previously a single instance role was supported, which was always `secondary`
moved {
  from = aws_iam_role_policy.instance_assume
  to   = aws_iam_role_policy.instance_assume["secondary"]
}
