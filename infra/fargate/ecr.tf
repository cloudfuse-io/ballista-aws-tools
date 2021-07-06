resource "aws_ecr_repository" "image_repository" {
  name                 = "${module.env.module_name}-${var.name}-${module.env.stage}"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = false
  }
}

resource "null_resource" "ballista_standalone_push" {
  count = var.push_image ? 1 : 0

  triggers = {
    always_run = timestamp()
  }

  provisioner "local-exec" {
    command = <<EOT
      docker tag "${var.source_docker_image}" "${aws_ecr_repository.image_repository.repository_url}"
      docker push "${aws_ecr_repository.image_repository.repository_url}"
    EOT
  }
}