resource "aws_efs_file_system" "efs_test_data" {
  tags = {
    Name = "${module.env.module_name}-efs-test-data-${module.env.stage}"
  }
}

resource "aws_security_group" "efs_sg" {
  name        = "${module.env.tags["module"]}-efs-${module.env.stage}"
  description = "allow inbound nfs access"
  vpc_id      = module.vpc.vpc_id

  ingress {
    protocol    = "TCP"
    from_port   = 2049
    to_port     = 2049
    cidr_blocks = module.env.subnet_cidrs
  }

  tags = module.env.tags
}

resource "aws_efs_mount_target" "alpha" {
  count = length(module.env.subnet_cidrs)
  file_system_id  = aws_efs_file_system.efs_test_data.id
  subnet_id       = module.vpc.public_subnets[count.index]
  security_groups = [aws_security_group.efs_sg.id]
}
