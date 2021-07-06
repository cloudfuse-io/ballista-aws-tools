# AWS tooling to deploy Apache Ballista

## What is Ballista?

Ballista is a distributed engine based on Datafusion and powered by Apache Arrow. 

For more information, please visit https://github.com/apache/arrow-datafusion.

## What is this repository?

This repository provides some tooling to automate the deployment of Ballista on AWS.
- Currently supports deployment on AWS Fargate
- Data is made accessible through AWS EFS
- Terraform is used to automate the cloud deployment

## Architecture

The system is composed of the following parts:
- A "standalone" Fargate task that contains both a Ballista scheduler and an executor
- Optional "executor" Fargate tasks
- A "trigger" lambda function that creates and discovers the "standalone" and "executor" tasks and submits queries to it
- a "copy-data" lambda function that copies data from S3 to EFS to make it available to the Ballista cluster and the trigger lambda.

## How to use it

You need Docker, the AWS CLI V2 and terraform to be installed.

Optionally, you can create a configuration file at the root called `default.env` with the AWS configs. Otherwise, you will be prompted repeatedly by terraform.
```makefile
# AWS region where Ballista will be deployed
REGION:=us-east-2
# AWS credentials profile for the Terraform deployment of Ballista
DEPLOY_PROFILE:=xxx
# AWS credentials profile for the Terraform state storage
BACKEND_PROFILE:=yyy
# Stage of the stack
STAGE:=dev
```

You can then use make commands to deploy the stack:
- `make docker-login` to login to AWS ECR
- `make deploy-all` to run the terraform deployment (an AWS bucket is required to act as terraform backend)
- `make run-integ-aws` to run the trigger lambda
- `make destroy` to clean up the AWS account.