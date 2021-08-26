SHELL := /bin/bash # Use bash syntax
GIT_REVISION = `git rev-parse --short HEAD``git diff --quiet HEAD -- || echo "-dirty"`

# create a default.env config file to avoid being asked configs over and over
include default.env
DEPLOY_PROFILE ?= $(eval DEPLOY_PROFILE := $(shell bash -c 'read -p "Deploy Profile: " input; echo $$input'))$(DEPLOY_PROFILE)
BACKEND_PROFILE ?= $(eval BACKEND_PROFILE := $(shell bash -c 'read -p "Backend Profile: " input; echo $$input'))$(BACKEND_PROFILE)
STAGE ?= $(eval STAGE := $(shell bash -c 'read -p "Stage: " input; echo $$input'))$(STAGE)
REGION ?= $(eval REGION := $(shell bash -c 'read -p "Region: " input; echo $$input'))$(REGION)

# use local install of terraform
terraform = AWS_PROFILE=${BACKEND_PROFILE} terraform

## CONFIGURATION

check-dirty:
	@git diff --quiet HEAD -- || { echo "ERROR: commit first, or use 'make force-deploy' to deploy dirty"; exit 1; }

ask-run-target:
	@echo "Running with profile ${DEPLOY_PROFILE}..."

ask-deploy-target:
	@echo "Deploying ${GIT_REVISION} in ${STAGE} with profile ${DEPLOY_PROFILE}, backend profile ${BACKEND_PROFILE}..."

# login to ECR
docker-login:
	aws ecr get-login-password --region "${REGION}" --profile=${DEPLOY_PROFILE} | \
	docker login --username AWS --password-stdin \
		"$(shell aws sts get-caller-identity --profile=${DEPLOY_PROFILE} --query 'Account' --output text).dkr.ecr.${REGION}.amazonaws.com"

## BUILD CONTAINERS AND LAMBDAS

build:
	cd rust; cargo build

rust/target/docker/%.zip: $(shell find rust/src -type f) rust/Cargo.toml docker/Dockerfile
	mkdir -p ./rust/target/docker
	DOCKER_BUILDKIT=1 docker build \
		-f docker/Dockerfile \
		--build-arg BIN_NAME=$* \
		--target export-stage \
		--output ./rust/target/docker \
		.

# package the lambda functions
package-lambdas: rust/target/docker/trigger.zip

# create standalone (scheduler+executor) container
package-standalone:
	DOCKER_BUILDKIT=1 docker build \
		-t cloudfuse/ballista-standalone:${GIT_REVISION} \
		-f docker/Dockerfile \
		--build-arg BIN_NAME=standalone \
		--build-arg PORT=50050 \
		--target runtime-stage \
		.

# create executor container
package-executor:
	DOCKER_BUILDKIT=1 docker build \
		-t cloudfuse/ballista-executor:${GIT_REVISION} \
		-f docker/Dockerfile \
		--build-arg BIN_NAME=executor \
		--build-arg PORT=50051 \
		--target runtime-stage \
		.

## MANAGE AWS INFRASTRUCTURE

# init terraform stack
init:
	@cd infra; ${terraform} init
	@cd infra; ${terraform} workspace new ${STAGE} &>/dev/null || echo "${STAGE} already exists"

# destroy terraform stack
destroy: ask-deploy-target
	cd infra; ${terraform} destroy \
		--var profile=${DEPLOY_PROFILE} \
		--var region_name=${REGION}

# deploy/update the terraform stack, requires to be logged in to ECR
deploy-all: ask-deploy-target package-standalone package-executor package-lambdas
	@echo "DEPLOYING ${GIT_REVISION} on ${STAGE}..."
	@cd infra; ${terraform} workspace select ${STAGE}
	@cd infra; ${terraform} apply \
		--var profile=${DEPLOY_PROFILE} \
		--var region_name=${REGION} \
		--var git_revision=${GIT_REVISION}
	@echo "${GIT_REVISION} DEPLOYED !!!"

## TPHC DATASET

# generate mock data for testing
generate-tpch-data:
	docker build \
		-t cloudfuse/ballista-tpchgen:v1 \
		-f docker/tpch/Dockerfile \
		.
	mkdir -p data
	docker run -v `pwd`/data:/data -it --rm cloudfuse/ballista-tpchgen:v1

# copy mock data from S3 to EFS
copy-to-efs:
	aws lambda invoke \
		--function-name $(shell bash -c 'cd infra; ${terraform} output copy_data_lambda_name') \
		--log-type Tail \
		--region ${REGION} \
		--profile ${DEPLOY_PROFILE} \
		--query 'LogResult' \
		--output text \
		/dev/null | base64 -d

## RUN QUERIES

# start the cluster locally with docker compose and run a query
run-integ-docker: ask-run-target
	COMPOSE_DOCKER_CLI_BUILD=1 DOCKER_BUILDKIT=1 docker-compose -f docker/docker-compose.yml build
	COMPOSE_DOCKER_CLI_BUILD=1 DOCKER_BUILDKIT=1 AWS_PROFILE=${DEPLOY_PROFILE} docker-compose -f docker/docker-compose.yml up --abort-on-container-exit

# call the trigger lambda to start the cluster and run a query
run-integ-aws: ask-run-target
	AWS_MAX_ATTEMPTS=1 aws lambda invoke \
			--function-name $(shell bash -c 'cd infra; ${terraform} output trigger_lambda_name') \
			--log-type Tail \
			--region ${REGION} \
			--profile ${DEPLOY_PROFILE} \
			--query 'LogResult' \
			--output text \
			/dev/null | base64 -d
