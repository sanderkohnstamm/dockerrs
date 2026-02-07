use bollard::container::{
    KillContainerOptions, ListContainersOptions, LogsOptions, RemoveContainerOptions,
    StartContainerOptions,
};
use bollard::network::ListNetworksOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::app::{DockerAction, DockerEvent};

/// Spawns a background task that polls Docker every 2 seconds for containers and networks,
/// and processes actions sent from the UI.
pub fn spawn_docker_poller(
    event_tx: mpsc::Sender<DockerEvent>,
    mut action_rx: mpsc::Receiver<DockerAction>,
) {
    tokio::spawn(async move {
        let docker = match Docker::connect_with_local_defaults() {
            Ok(d) => d,
            Err(e) => {
                let _ = event_tx
                    .send(DockerEvent::ActionResult {
                        success: false,
                        message: format!("Failed to connect to Docker: {}", e),
                    })
                    .await;
                return;
            }
        };

        let mut poll_interval = tokio::time::interval(Duration::from_secs(2));
        poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = poll_interval.tick() => {
                    // List containers
                    if let Ok(containers) = docker.list_containers(Some(ListContainersOptions::<String> {
                        all: true,
                        ..Default::default()
                    })).await {
                        let _ = event_tx.send(DockerEvent::ContainersUpdated(containers)).await;
                    }

                    // List networks
                    if let Ok(networks) = docker.list_networks(Some(ListNetworksOptions::<String> {
                        filters: HashMap::new(),
                    })).await {
                        let _ = event_tx.send(DockerEvent::NetworksUpdated(networks)).await;
                    }
                }

                Some(action) = action_rx.recv() => {
                    match action {
                        DockerAction::Start(id) => {
                            let result = docker.start_container(&id, None::<StartContainerOptions<String>>).await;
                            let (success, message) = match result {
                                Ok(_) => (true, format!("Started container {}", short_id(&id))),
                                Err(e) => (false, format!("Start failed: {}", e)),
                            };
                            let _ = event_tx.send(DockerEvent::ActionResult { success, message }).await;
                        }
                        DockerAction::Stop(id) => {
                            let result = docker.stop_container(&id, None).await;
                            let (success, message) = match result {
                                Ok(_) => (true, format!("Stopped container {}", short_id(&id))),
                                Err(e) => (false, format!("Stop failed: {}", e)),
                            };
                            let _ = event_tx.send(DockerEvent::ActionResult { success, message }).await;
                        }
                        DockerAction::Kill(id) => {
                            let result = docker.kill_container(&id, Some(KillContainerOptions { signal: "SIGKILL" })).await;
                            let (success, message) = match result {
                                Ok(_) => (true, format!("Killed container {}", short_id(&id))),
                                Err(e) => (false, format!("Kill failed: {}", e)),
                            };
                            let _ = event_tx.send(DockerEvent::ActionResult { success, message }).await;
                        }
                        DockerAction::Remove(id) => {
                            let result = docker.remove_container(&id, Some(RemoveContainerOptions {
                                force: true,
                                ..Default::default()
                            })).await;
                            let (success, message) = match result {
                                Ok(_) => (true, format!("Removed container {}", short_id(&id))),
                                Err(e) => (false, format!("Remove failed: {}", e)),
                            };
                            let _ = event_tx.send(DockerEvent::ActionResult { success, message }).await;
                        }
                        DockerAction::StreamLogs { container_id } => {
                            spawn_log_stream(&docker, &container_id, event_tx.clone());
                        }
                        DockerAction::StopLogStream => {
                            // Log stream tasks check a separate mechanism (dropped on new stream)
                        }
                    }
                }
            }
        }
    });
}

/// Spawns a task that streams logs from a container and sends them as events.
fn spawn_log_stream(docker: &Docker, container_id: &str, event_tx: mpsc::Sender<DockerEvent>) {
    let docker = docker.clone();
    let container_id = container_id.to_string();

    tokio::spawn(async move {
        let options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            tail: "200".to_string(),
            ..Default::default()
        };

        let mut stream = docker.logs(&container_id, Some(options));

        while let Some(result) = stream.next().await {
            match result {
                Ok(output) => {
                    let text = output.to_string();
                    for line in text.lines() {
                        if event_tx.send(DockerEvent::LogLine(line.to_string())).await.is_err() {
                            return;
                        }
                    }
                }
                Err(_) => break,
            }
        }

        let _ = event_tx.send(DockerEvent::LogStreamEnded).await;
    });
}

fn short_id(id: &str) -> &str {
    if id.len() > 12 {
        &id[..12]
    } else {
        id
    }
}
