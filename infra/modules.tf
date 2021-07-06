##### BALLISTA #####

resource "aws_ecr_repository" "ballista_repo" {
  name                 = "${module.env.module_name}-ballista-standalone-${module.env.stage}"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = false
  }
}

resource "null_resource" "ballista_standalone_push" {
  count = var.push_ballista ? 1 : 0

  triggers = {
    always_run = timestamp()
  }

  provisioner "local-exec" {
    command = <<EOT
      docker tag "cloudfuse/ballista-standalone:${var.git_revision}" "${aws_ecr_repository.ballista_repo.repository_url}:${var.git_revision}"
      docker push "${aws_ecr_repository.ballista_repo.repository_url}:${var.git_revision}"
    EOT
  }
}

module "ballista" {
  source = "./fargate"

  name                        = "ballista"
  region_name                 = var.region_name
  vpc_id                      = module.vpc.vpc_id
  task_cpu                    = 2048
  task_memory                 = 4096
  ecs_cluster_id              = aws_ecs_cluster.ballista_cluster.id
  ecs_cluster_name            = aws_ecs_cluster.ballista_cluster.name
  ecs_task_execution_role_arn = aws_iam_role.ecs_task_execution_role.arn
  docker_image                = "${aws_ecr_repository.ballista_repo.repository_url}:${var.git_revision}"
  subnets                     = module.vpc.public_subnets

  attach_efs     = true
  file_system_id = aws_efs_file_system.efs_test_data.id

  environment = [{
    name  = "GIT_REVISION"
    value = var.git_revision
    }, {
    name  = "AWS_REGION"
    value = var.region_name
  }]
}

##### TRIGGER #####

module "trigger" {
  source = "./lambda"

  function_base_name = "trigger"
  region_name        = var.region_name
  filename           = "../rust/target/docker/trigger.zip"
  memory_size        = 3008
  timeout            = 30

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
    GIT_REVISION       = var.git_revision
    BALLISTA_CLUSTER_NAME = aws_ecs_cluster.ballista_cluster.name
    BALLISTA_TASK_SG_ID   = module.ballista.task_security_group_id
    BALLISTA_TASK_DEF_ARN = module.ballista.task_definition_arn
    PUBLIC_SUBNETS     = join(",", module.vpc.public_subnets)
  }
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
  timeout            = 120
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
    GIT_REVISION       = var.git_revision
  }
}
