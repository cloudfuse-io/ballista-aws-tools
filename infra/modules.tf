##### BALLISTA #####

module "ballista_standalone" {
  source = "./fargate"

  name                        = "ballista-standalone"
  region_name                 = var.region_name
  vpc_id                      = module.vpc.vpc_id
  task_cpu                    = 2048
  task_memory                 = 4096
  ecs_cluster_id              = aws_ecs_cluster.ballista_cluster.id
  ecs_cluster_name            = aws_ecs_cluster.ballista_cluster.name
  ecs_task_execution_role_arn = aws_iam_role.ecs_task_execution_role.arn
  source_docker_image         = "cloudfuse/ballista-standalone:${var.git_revision}"
  push_image                  = var.push_ballista
  subnets                     = module.vpc.public_subnets

  attach_efs     = true
  file_system_id = aws_efs_file_system.efs_test_data.id

  environment = [{
    name  = "GIT_REVISION"
    value = var.git_revision
    }, {
    name  = "AWS_REGION"
    value = var.region_name
    }, {
    name  = "RUST_LOG"
    value = "info"
  }]
}

module "ballista_executor" {
  source = "./fargate"

  name                        = "ballista-executor"
  region_name                 = var.region_name
  vpc_id                      = module.vpc.vpc_id
  task_cpu                    = 2048
  task_memory                 = 4096
  ecs_cluster_id              = aws_ecs_cluster.ballista_cluster.id
  ecs_cluster_name            = aws_ecs_cluster.ballista_cluster.name
  ecs_task_execution_role_arn = aws_iam_role.ecs_task_execution_role.arn
  source_docker_image         = "cloudfuse/ballista-executor:${var.git_revision}"
  push_image                  = var.push_ballista
  subnets                     = module.vpc.public_subnets

  additional_policies = [
    # aws_iam_policy.s3-additional-policy.arn,
    aws_iam_policy.fargate-additional-policy.arn
    # aws_iam_policy.lambda-additional-policy.arn
  ]

  attach_efs     = true
  file_system_id = aws_efs_file_system.efs_test_data.id

  environment = [{
    name  = "GIT_REVISION"
    value = var.git_revision
    }, {
    name  = "AWS_REGION"
    value = var.region_name
    }, {
    name  = "BALLISTA_EXECUTOR_CLUSTER_NAME"
    value = aws_ecs_cluster.ballista_cluster.name
    }, {
    name  = "BALLISTA_EXECUTOR_SCHEDULER_TASK_DEF_ARN"
    value = module.ballista_standalone.task_definition_arn
    }, {
    name  = "RUST_LOG"
    value = "info"
  }]
}

##### TRIGGER #####

module "trigger" {
  source = "./lambda"

  function_base_name = "trigger"
  region_name        = var.region_name
  filename           = "../rust/target/docker/trigger.zip"
  memory_size        = 3008
  timeout            = 900

  in_vpc  = true
  vpc_id  = module.vpc.vpc_id
  subnets = module.vpc.public_subnets

  attach_efs     = true
  file_system_id = aws_efs_file_system.efs_test_data.id

  additional_policies = [
    # aws_iam_policy.s3-additional-policy.arn,
    aws_iam_policy.fargate-additional-policy.arn
    # aws_iam_policy.lambda-additional-policy.arn
  ]

  environment = {
    RUST_LOG                                 = "info"
    GIT_REVISION                             = var.git_revision
    BALLISTA_TRIGGER_CLUSTER_NAME            = aws_ecs_cluster.ballista_cluster.name
    BALLISTA_TRIGGER_STANDALONE_TASK_SG_ID   = module.ballista_standalone.task_security_group_id
    BALLISTA_TRIGGER_STANDALONE_TASK_DEF_ARN = module.ballista_standalone.task_definition_arn
    BALLISTA_TRIGGER_EXECUTOR_TASK_SG_ID     = module.ballista_executor.task_security_group_id
    BALLISTA_TRIGGER_EXECUTOR_TASK_DEF_ARN   = module.ballista_executor.task_definition_arn
    BALLISTA_TRIGGER_SUBNETS                 = join(",", module.vpc.public_subnets)
  }

  # lambda attached to EFS will fail to create if the mount points are not ready
  depends_on = [
    aws_efs_mount_target.alpha,
  ]
}

data "archive_file" "copy_function_zip" {
  type        = "zip"
  source_file = "copy-data.py"
  output_path = ".terraform/copy-data.zip"
}

module "copy_data" {
  source = "./lambda"

  function_base_name = "copy-data"
  region_name        = var.region_name
  filename           = ".terraform/copy-data.zip"
  memory_size        = 3008
  timeout            = 900
  handler            = "copy-data.lambda_handler"
  runtime            = "python3.8"

  in_vpc  = true
  vpc_id  = module.vpc.vpc_id
  subnets = module.vpc.public_subnets

  attach_efs     = true
  file_system_id = aws_efs_file_system.efs_test_data.id

  additional_policies = [
    aws_iam_policy.s3-additional-policy.arn,
  ]

  environment = {
    GIT_REVISION = var.git_revision
  }

  # lambda attached to EFS will fail to create if the mount points are not ready
  depends_on = [
    aws_efs_mount_target.alpha,
  ]
}
