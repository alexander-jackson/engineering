module "application_role" {
  source = "./modules/application-role"

  for_each = {
    certmanager = jsonencode({
      Statement = [
        {
          Action   = ["route53:ListHostedZones", "route53:GetChange"]
          Effect   = "Allow"
          Resource = "*"
        },
        {
          Action = ["route53:ChangeResourceRecordSets"]
          Effect = "Allow"
          Resource = [
            format("arn:aws:route53:::hostedzone/%s", aws_route53_zone.opentracker.id),
            format("arn:aws:route53:::hostedzone/%s", aws_route53_zone.forkup.id),
          ]
        },
        {
          Action = ["s3:PutObject"]
          Effect = "Allow"
          Resource = [
            format("%s/f2/certificates/*/fullchain.pem", module.config_bucket.arn),
            format("%s/f2/certificates/*/privkey.pem", module.config_bucket.arn),
          ]
        },
      ]
    })

    pgmanager = jsonencode({
      Statement = [
        {
          Action   = ["s3:PutObject"]
          Effect   = "Allow"
          Resource = format("%s/*", module.postgres_backups_bucket.arn)
        },
      ]
    })

    uptime = jsonencode({
      Statement = [
        {
          Action   = ["sns:Publish"]
          Effect   = "Allow"
          Resource = aws_sns_topic.outages.arn
        },
      ]
    })
  }

  application_name   = each.key
  instance_role_name = module.primary.role_name
  policy_json        = each.value
}
