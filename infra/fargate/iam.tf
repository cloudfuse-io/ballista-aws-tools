resource "aws_iam_role" "ecs_task_role" {
  name = "${module.env.tags["module"]}_${var.name}_${module.env.stage}_${var.region_name}"

  assume_role_policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "",
      "Effect": "Allow",
      "Principal": {
        "Service": "ecs-tasks.amazonaws.com"
      },
      "Action": "sts:AssumeRole"
    }
  ]
}
EOF


  tags = module.env.tags
}

resource "aws_iam_role_policy_attachment" "additional-attachments" {
  count = length(var.additional_policies)

  role       = aws_iam_role.ecs_task_role.name
  policy_arn = var.additional_policies[count.index]
}
