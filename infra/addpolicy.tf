resource "aws_iam_policy" "s3-additional-policy" {
  name        = "${module.env.module_name}_s3_access_${var.region_name}_${module.env.stage}"
  description = "additional policy for s3 access"
  policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Action": [
        "s3:*"
      ],
      "Resource": "*",
      "Effect": "Allow"
    }
  ]
}
EOF
}

resource "aws_iam_policy" "fargate-additional-policy" {
  name        = "${module.env.module_name}_fargate_access_${var.region_name}_${module.env.stage}"
  description = "additional policy for fargate access"

  policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Action": [
        "ecs:DescribeTasks",
        "ecs:ListTasks"
      ],
      "Resource": "*",
      "Condition" : { "StringEquals" : { "ecs:cluster" : "${aws_ecs_cluster.ballista_cluster.arn}" }},
      "Effect": "Allow"
    },
    {
      "Action": [
        "ecs:RunTask",
        "ecs:StartTask"
      ],
      "Resource": [
        "${module.ballista_standalone.task_definition_arn}",
        "${module.ballista_executor.task_definition_arn}"
      ],
      "Effect": "Allow"
    },
    {
      "Action": [
        "ecs:DescribeTaskDefinition"
      ],
      "Resource": "*",
      "Effect": "Allow"
    },
    {
      "Action": [
        "iam:PassRole"
      ],
      "Resource": "${aws_iam_role.ecs_task_execution_role.arn}",
      "Effect": "Allow"
    }
  ]
}
EOF
}

# resource "aws_iam_policy" "lambda-additional-policy" {
#   name        = "${module.env.module_name}_lambda_access_${var.region_name}_${module.env.stage}"
#   description = "additional policy for lambda access"
#   policy = <<EOF
# {
#   "Version": "2012-10-17",
#   "Statement": [
#     {
#       "Action": [
#         "lambda:InvokeFunction"
#       ],
#       "Resource": "${module.hbee.lambda_arn}",
#       "Effect": "Allow"
#     }
#   ]
# }
# EOF
# }