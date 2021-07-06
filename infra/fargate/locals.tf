locals {
  efs_mount_point = {
      "containerPath": "/mnt/data",
      "sourceVolume": "efs-data"
  }
  container_definition = [
    {
      cpu         = var.task_cpu
      image       = aws_ecr_repository.image_repository.repository_url
      memory      = var.task_memory
      name        = var.name
      essential   = true
      mountPoints = []
      portMappings = [
        {
          containerPort = 50050
          hostPort      = 50050
          protocol      = "tcp"
        },
        {
          containerPort = 50051
          hostPort      = 50051
          protocol      = "tcp"
        },
      ]
      mountPoints = var.attach_efs ? [ local.efs_mount_point ] : []
      volumesFrom = []
      environment = var.environment
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          awslogs-group         = aws_cloudwatch_log_group.fargate_logging.name
          awslogs-region        = var.region_name
          awslogs-stream-prefix = "ecs"
        }
      }
    },
  ]
}
