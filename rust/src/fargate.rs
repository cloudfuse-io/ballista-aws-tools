use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use log::info;
use rusoto_core::Region;
use rusoto_ecs::{
    AwsVpcConfiguration, DescribeTaskDefinitionRequest, DescribeTasksRequest, Ecs, EcsClient,
    ListTasksRequest, ListTasksResponse, NetworkConfiguration, RunTaskRequest,
};
use tokio::time::timeout;

pub struct FargateCreationClient {
    client: Arc<EcsClient>,
    cluster_name: String,
}

impl FargateCreationClient {
    pub fn try_new(cluster_name: String) -> Result<Self> {
        let aws_region = env::var("AWS_REGION")?;
        Ok(Self {
            client: new_client(&aws_region),
            cluster_name,
        })
    }
}

impl FargateCreationClient {
    /// Create a new fargate task and returns private IP.
    /// The task might not be ready to receive requests yet.
    /// TODO make it possible to create multiple tasks
    pub async fn create_new(
        &self,
        task_def_arn: String,
        task_sg_id: String,
        subnets: Vec<String>,
    ) -> Result<String> {
        let start = Instant::now();

        let mut task_arn = self.get_existing_task(task_def_arn.clone()).await;

        if task_arn.is_none() {
            task_arn = Some(
                self.start_task(task_def_arn.clone(), task_sg_id.clone(), subnets)
                    .await?,
            );
            info!("task started");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let result = self.wait_for_provisioning(task_arn.unwrap()).await?;

        info!("took {}ms to create/find task", start.elapsed().as_millis());

        Ok(result)
    }

    /// Get the family of the given task definition.
    /// TODO better error management
    async fn get_task_family(&self, task_def_arn: String) -> Result<String> {
        let request = DescribeTaskDefinitionRequest {
            include: None,
            task_definition: task_def_arn,
        };

        let result = timeout(
            Duration::from_secs(2),
            self.client.describe_task_definition(request),
        )
        .await??;

        // if request for existing task failed for any reason, return None
        result
            .task_definition
            .ok_or(anyhow!("Task definition object should not be undefined"))?
            .family
            .ok_or(anyhow!("Task definition family should not be undefined"))
    }

    /// Get existing task ARN if there is at least one.
    /// Picks the first one returned by the API.
    /// TODO better error management
    pub async fn get_existing_task(&self, task_def_arn: String) -> Option<String> {
        let family = self.get_task_family(task_def_arn).await.unwrap();

        let request = ListTasksRequest {
            cluster: Some(self.cluster_name.clone()),
            container_instance: None,
            desired_status: Some("RUNNING".to_owned()),
            family: Some(family),
            launch_type: None,
            max_results: Some(1),
            next_token: None,
            service_name: None,
            started_by: None,
        };

        let result = timeout(Duration::from_secs(2), self.client.list_tasks(request)).await;

        // if request for existing task failed for any reason, return None
        match result {
            Ok(Ok(ListTasksResponse {
                task_arns: Some(arns),
                ..
            })) if arns.len() > 0 => Some(arns[0].clone()),
            _ => None,
        }
    }

    /// Start new task and return its arn
    /// TODO better error management
    async fn start_task(
        &self,
        task_def_arn: String,
        security_group: String,
        subnets: Vec<String>,
    ) -> Result<String> {
        let input = RunTaskRequest {
            group: None,
            task_definition: task_def_arn,
            count: Some(1),
            cluster: Some(self.cluster_name.clone()),
            network_configuration: Some(NetworkConfiguration {
                awsvpc_configuration: Some(AwsVpcConfiguration {
                    assign_public_ip: Some("ENABLED".to_owned()),
                    subnets,
                    security_groups: Some(vec![security_group]),
                }),
            }),
            enable_ecs_managed_tags: None,
            capacity_provider_strategy: None,
            placement_constraints: None,
            placement_strategy: None,
            platform_version: None,
            launch_type: None,
            overrides: None,
            propagate_tags: None,
            reference_id: None,
            started_by: None,
            tags: None,
        };
        let result = timeout(Duration::from_secs(5), self.client.run_task(input)).await??;
        if let Some(failures) = result.failures {
            if failures.len() > 0 {
                return Err(anyhow!(
                    "An error occured with AWS Fargate task creation: {:?}",
                    failures
                ));
            }
        }

        Ok(result
            .tasks
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .task_arn
            .unwrap())
    }

    /// Wait for the given task to be provisioned and attributed a private IP
    /// TODO better error management
    /// TODO fargate container lifecycle
    pub async fn wait_for_provisioning(&self, task_arn: String) -> Result<String> {
        loop {
            let input = DescribeTasksRequest {
                cluster: Some(self.cluster_name.clone()),
                include: None,
                tasks: vec![task_arn.clone()],
            };
            let description = self.client.describe_tasks(input).await?.tasks.unwrap();

            let attachment_props = description[0].attachments.as_ref().unwrap()[0]
                .details
                .as_ref()
                .unwrap();

            for prop in attachment_props {
                if let Some(ref key) = prop.name {
                    if key == "privateIPv4Address" && prop.value.as_ref().is_some() {
                        return Ok(prop.value.as_ref().unwrap().clone());
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

//// Lambda Client ////

fn new_client(region: &str) -> Arc<EcsClient> {
    let region = Region::from_str(region).unwrap();
    Arc::new(EcsClient::new(region))
}
