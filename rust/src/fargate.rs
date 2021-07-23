use std::env;
use std::str;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use futures::{Future, StreamExt, TryStreamExt};
use hyper::{body::to_bytes, Body, Client, Uri};
use log::info;
use rusoto_core::Region;
use rusoto_ecs::{
    AwsVpcConfiguration, DescribeTaskDefinitionRequest, DescribeTasksRequest, Ecs, EcsClient,
    ListTasksRequest, NetworkConfiguration, RunTaskRequest,
};
use serde::Deserialize;
use tokio::time::timeout;

pub struct FargateCreationClient {
    client: Arc<EcsClient>,
    cluster_name: String,
}

async fn api_timeout<T, S, E>(future: T) -> Result<S>
where
    T: Future<Output = std::result::Result<S, E>>,
    E: std::error::Error + Sync + Send + 'static,
{
    Ok(timeout(Duration::from_secs(2), future)
        .await
        .context("Query to Fargate API timed out")??)
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
    pub async fn get_or_provision(
        &self,
        task_def_arn: String,
        task_sg_id: String,
        subnets: Vec<String>,
        count: usize,
    ) -> Result<Vec<String>> {
        let start = Instant::now();

        let mut task_arns = self.get_existing_tasks(task_def_arn.clone()).await?;

        task_arns.truncate(count);

        let missing_task_count = count.saturating_sub(task_arns.len());

        if missing_task_count > 0 {
            let future_tasks = (0..missing_task_count).map(|_| {
                self.start_task(task_def_arn.clone(), task_sg_id.clone(), subnets.clone())
            });
            let mut new_task_arns = futures::stream::iter(future_tasks)
                .buffer_unordered(5)
                .try_collect::<Vec<_>>()
                .await?;
            task_arns.append(&mut new_task_arns);

            info!("{} task started", missing_task_count);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let result = self.wait_for_provisioning(task_arns).await?;

        info!(
            "took {}ms to create/find tasks",
            start.elapsed().as_millis()
        );

        Ok(result)
    }

    /// Get the family of the given task definition.
    /// TODO better error management
    async fn get_task_family(&self, task_def_arn: String) -> Result<String> {
        let request = DescribeTaskDefinitionRequest {
            include: None,
            task_definition: task_def_arn,
        };

        let result = api_timeout(self.client.describe_task_definition(request)).await?;

        // if request for existing task failed for any reason, return None
        result
            .task_definition
            .ok_or(anyhow!("Task definition object should not be undefined"))?
            .family
            .ok_or(anyhow!("Task definition family should not be undefined"))
    }

    /// Get existing task ARNs.
    pub async fn get_existing_tasks(&self, task_def_arn: String) -> Result<Vec<String>> {
        let family = self.get_task_family(task_def_arn).await.unwrap();

        let request = ListTasksRequest {
            cluster: Some(self.cluster_name.clone()),
            container_instance: None,
            desired_status: Some("RUNNING".to_owned()),
            family: Some(family),
            launch_type: None,
            max_results: None,
            next_token: None,
            service_name: None,
            started_by: None,
        };

        api_timeout(self.client.list_tasks(request))
            .await
            .context("failed to call Fargate list_tasks")?
            .task_arns
            .context("Task arn list was undefined")
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
        let result = api_timeout(self.client.run_task(input)).await?;
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
    pub async fn wait_for_provisioning(&self, task_arns: Vec<String>) -> Result<Vec<String>> {
        loop {
            let input = DescribeTasksRequest {
                cluster: Some(self.cluster_name.clone()),
                include: None,
                tasks: task_arns.clone(),
            };
            let description = api_timeout(self.client.describe_tasks(input))
                .await?
                .tasks
                .unwrap();

            let mut ips = vec![];

            for _ in 0..task_arns.len() {
                let attachment_props = description[0].attachments.as_ref().unwrap()[0]
                    .details
                    .as_ref()
                    .unwrap();

                for prop in attachment_props {
                    if let Some(ref key) = prop.name {
                        if key == "privateIPv4Address" && prop.value.as_ref().is_some() {
                            ips.push(prop.value.as_ref().unwrap().clone());
                        }
                    }
                }
            }
            if ips.len() == task_arns.len() {
                return Ok(ips);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

//// Fargate Client ////

fn new_client(region: &str) -> Arc<EcsClient> {
    let region = Region::from_str(region).unwrap();
    Arc::new(EcsClient::new(region))
}

////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Deserialize)]
pub struct FargateNetwork {
    #[serde(rename(deserialize = "IPv4Addresses"))]
    pub ipv4_addresses: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FargateContainer {
    #[serde(rename(deserialize = "Networks"))]
    pub networks: Vec<FargateNetwork>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FargateMetadata {
    #[serde(rename(deserialize = "Containers"))]
    pub containers: Vec<FargateContainer>,
}

/// get the external IP for the current Fargate task
pub async fn get_fargate_task_external_host() -> Result<String> {
    let matadata_endpoint = env::var("ECS_CONTAINER_METADATA_URI_V4")?;
    let uri: Uri = (matadata_endpoint + "/task").parse()?;
    let client = Client::new();
    loop {
        let resp = client.get(uri.clone()).await?;
        let body: Body = resp.into_body();
        let body_bytes = to_bytes(body).await?;
        let metadata: FargateMetadata = serde_json::from_slice(&body_bytes).with_context(|| {
            format!(
                "Impossible to parse task metadata: {}",
                str::from_utf8(&body_bytes).unwrap()
            )
        })?;
        if metadata.containers.len() > 0
            && metadata.containers[0].networks.len() > 0
            && metadata.containers[0].networks[0].ipv4_addresses.len() > 0
        {
            return Ok(metadata.containers[0].networks[0].ipv4_addresses[0].clone());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
