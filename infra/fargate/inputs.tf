module "env" {
  source = "../env"
}

variable "region_name" {}

variable "name" {}

variable "environment" {
  type = list(object({
    name  = string
    value = string
  }))
}

variable "vpc_id" {}

variable "task_cpu" {}

variable "task_memory" {}

variable "ecs_cluster_id" {}

variable "ecs_cluster_name" {}

variable "ecs_task_execution_role_arn" {}

variable "source_docker_image" {}

variable "subnets" {}

variable "push_image" {
  description = "Set to false if image does not need to be updated"
  default     = true
}

variable "attach_efs" {
  description = "Set this to true if the lambda use EFS"
  default     = false
}

variable "file_system_id" {
  default = ""
}
