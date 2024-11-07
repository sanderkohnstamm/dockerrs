use std::{collections::HashMap, time::Duration};

use bollard::{
    container::{KillContainerOptions, ListContainersOptions, LogsOptions, RemoveContainerOptions},
    secret::ContainerSummary,
    Docker,
};
use futures_util::StreamExt;
use tokio::sync::mpsc::Sender;

use tokio::time::sleep;

pub struct DockerConnection {
    docker: Docker,
    sender: Sender<HashMap<String, (ContainerSummary, String)>>,
}
impl DockerConnection {
    pub fn new(
        docker: Docker,
        sender: Sender<HashMap<String, (ContainerSummary, String)>>,
    ) -> Self {
        let conn = Self { docker, sender };
        let log_options: LogsOptions<String> = LogsOptions::<String> {
            follow: false,
            stdout: true,
            stderr: true,
            tail: "1000".to_string(),
            ..Default::default()
        };
        conn.spawn_container_listener(log_options);
        conn
    }

    pub fn spawn_container_listener(&self, log_options: LogsOptions<String>) {
        let docker = self.docker.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            loop {
                let containers = docker
                    .list_containers(Some(ListContainersOptions::<String> {
                        all: true, // You may want to see all containers, not just running ones
                        ..Default::default()
                    }))
                    .await
                    .expect("Failed to list containers");

                let mut summaries = HashMap::new();

                for container in &containers {
                    if let Some(id) = &container.id {
                        let mut logs = String::new();
                        let mut log_stream = docker.logs(id, Some(log_options.clone()));

                        while let Some(chunk) = log_stream.next().await {
                            if let Ok(log) = chunk {
                                logs.push_str(&String::from_utf8_lossy(&log.into_bytes()));
                            }
                        }

                        let name = container.names.as_ref().map_or_else(
                            || "Unnamed Container".to_string(),
                            |names| names.join(", "),
                        );
                        summaries.insert(name, (container.clone(), logs));
                    }
                }

                if sender.send(summaries).await.is_err() {
                    eprintln!("Failed to send container logs");
                    break;
                }
                sleep(Duration::from_millis(50)).await;
            }
        });
    }

    pub async fn logs(&self, container_id: &str) -> String {
        let log_options: LogsOptions<String> = LogsOptions::<String> {
            follow: false,
            stdout: true,
            stderr: true,
            tail: "100".to_string(),
            ..Default::default()
        };
        let mut logs = String::new();
        let mut log_stream = self.docker.logs(container_id, Some(log_options));
        while let Some(chunk) = log_stream.next().await {
            if let Ok(log) = chunk {
                logs.push_str(&String::from_utf8_lossy(&log.into_bytes()));
            }
        }
        logs
    }
    pub fn remove_all_containers(&self) {
        let docker = self.docker.clone();
        let remove_options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };
        tokio::spawn(async move {
            let containers = docker
                .list_containers(Some(ListContainersOptions::<String> {
                    all: true,
                    ..Default::default()
                }))
                .await
                .expect("Failed to list containers");
            for container in containers {
                if let Some(id) = container.id {
                    if let Err(e) = docker
                        .remove_container(&id, Some(remove_options.clone()))
                        .await
                    {
                        eprintln!("Failed to stop container {}: {}", id, e);
                    }
                }
            }
        });
    }

    pub fn stop_all_containers(&self) {
        let docker = self.docker.clone();
        tokio::spawn(async move {
            let containers = docker
                .list_containers(Some(ListContainersOptions::<String> {
                    all: true,
                    ..Default::default()
                }))
                .await
                .expect("Failed to list containers");
            for container in containers {
                if let Some(id) = container.id {
                    if let Err(e) = docker.stop_container(&id, None).await {
                        eprintln!("Failed to stop container {}: {}", id, e);
                    }
                }
            }
        });
    }

    pub fn kill_all_containers(&self) {
        let docker = self.docker.clone();
        let kill_options = KillContainerOptions { signal: "SIGKILL" };

        tokio::spawn(async move {
            let containers = docker
                .list_containers(Some(ListContainersOptions::<String> {
                    all: true,
                    ..Default::default()
                }))
                .await
                .expect("Failed to list containers");
            for container in containers {
                if let Some(id) = container.id {
                    if let Err(e) = docker.kill_container(&id, Some(kill_options.clone())).await {
                        eprintln!("Failed to kill container {}: {}", id, e);
                    }
                }
            }
        });
    }

    pub fn stop_container(&self, container_id: Option<String>) {
        let Some(container_id) = container_id.clone() else {
            return;
        };
        let docker_clone = self.docker.clone();
        tokio::spawn(async move {
            if let Err(e) = docker_clone.stop_container(&container_id, None).await {
                eprintln!("Failed to stop container {}: {}", container_id, e);
            }
        });
    }

    pub fn start_container(&self, container_id: Option<String>) {
        let Some(container_id) = container_id.clone() else {
            return;
        };
        let docker = self.docker.clone();
        tokio::spawn(async move {
            if let Err(e) = docker.start_container::<String>(&container_id, None).await {
                eprintln!("Failed to start container {}: {}", container_id, e);
            }
        });
    }

    pub fn kill_container(&self, container_id: Option<String>) {
        let Some(container_id) = container_id.clone() else {
            return;
        };
        let docker_clone = self.docker.clone();
        let kill_options = KillContainerOptions { signal: "SIGKILL" };

        tokio::spawn(async move {
            if let Err(e) = docker_clone
                .kill_container(&container_id, Some(kill_options))
                .await
            {
                eprintln!("Failed to stop container {}: {}", container_id, e);
            }
        });
    }
    pub fn remove_container(&self, container_id: Option<String>) {
        let Some(container_id) = container_id.clone() else {
            return;
        };
        let remove_options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };
        let docker = self.docker.clone();
        tokio::spawn(async move {
            if let Err(e) = docker
                .remove_container(&container_id, Some(remove_options))
                .await
            {
                eprintln!("Failed to stop container {}: {}", container_id, e);
            }
        });
    }
}
