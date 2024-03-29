resource "aws_lambda_function" "lambda" {
  filename         = var.filename
  function_name    = "${module.env.tags["module"]}-${var.function_base_name}-${module.env.stage}"
  role             = aws_iam_role.lambda_role.arn
  handler          = var.handler
  memory_size      = var.memory_size
  timeout          = var.timeout
  source_code_hash = filebase64sha256(var.filename)
  runtime          = var.runtime

  environment {
    variables = merge(
      {
        STAGE = module.env.stage
      },
      var.environment
    )
  }

  dynamic "vpc_config" {
    for_each = var.in_vpc ? [1] : []
    content {
      security_group_ids = [aws_security_group.lambda_sg[0].id]
      subnet_ids         = var.subnets
    }
  }

  dynamic "file_system_config" {
    for_each = var.attach_efs ? [1] : []
    content {
      arn = aws_efs_access_point.access_point_for_lambda[0].arn
      local_mount_path = "/mnt/data"
    }
  }

  tags = module.env.tags
}

resource "aws_efs_access_point" "access_point_for_lambda" {
  count = var.attach_efs ? 1 : 0
  file_system_id = var.file_system_id

  root_directory {
    path = "/data"
    creation_info {
      owner_gid   = 1000
      owner_uid   = 1000
      permissions = "777"
    }
  }

  posix_user {
    gid = 1000
    uid = 1000
  }
}

resource "aws_lambda_function_event_invoke_config" "lambda_conf" {
  function_name                = aws_lambda_function.lambda.function_name
  maximum_event_age_in_seconds = 60
  maximum_retry_attempts       = 0
}


resource "aws_cloudwatch_log_group" "lambda_log_group" {
  name              = "/aws/lambda/${aws_lambda_function.lambda.function_name}"
  retention_in_days = 14
  tags              = module.env.tags
}

resource "aws_security_group" "lambda_sg" {
  count = var.in_vpc ? 1 : 0

  name        = "${module.env.tags["module"]}-${var.function_base_name}-${module.env.stage}"
  description = "allow outbound access"
  vpc_id      = var.vpc_id

  egress {
    protocol    = "-1"
    from_port   = 0
    to_port     = 0
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = module.env.tags
}
