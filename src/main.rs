pub mod docker_viewer_app;
pub mod utils;

use bollard::container::{ListContainersOptions, LogsOptions};
use bollard::secret::ContainerSummary;
use bollard::Docker;

use docker_viewer_app::{AppView, DockerViewerApp};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let log_options: LogsOptions<String> = LogsOptions::<String> {
        follow: false,
        stdout: true,
        stderr: true,
        tail: "100".to_string(),
        ..Default::default()
    };
    let (sender, receiver) = mpsc::channel(100);
    spawn_container_listener(sender, log_options);
    run_app(receiver);
}

fn spawn_container_listener(
    sender: Sender<HashMap<String, (ContainerSummary, String)>>,
    log_options: LogsOptions<String>,
) {
    tokio::spawn(async move {
        let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");

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

                    let name = container
                        .names
                        .as_ref()
                        .map_or_else(|| "Unnamed Container".to_string(), |names| names.join(", "));
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

fn run_app(receiver: Receiver<HashMap<String, (ContainerSummary, String)>>) {
    let options = eframe::NativeOptions::default();
    let mut app = DockerViewerApp {
        receiver,
        containers: HashMap::new(),
        selected_container: None,
        current_view: AppView::Containers,
        selected_compose_for_preview: None,
        compose_files: Vec::new(),
        dockerfiles: Vec::new(),
        selected_dockerfile_for_preview: None,
        docker_build_name: "add tag".to_owned(),
    };
    app.load_compose_files(Path::new("../"));
    app.load_dockerfiles(Path::new("../"));
    eframe::run_native("dockerrs", options, Box::new(|_cc| Box::new(app))).unwrap();
}
