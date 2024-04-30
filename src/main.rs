pub mod docker_viewer_app;

use bollard::container::{ListContainersOptions, LogsOptions};
use bollard::Docker;

use docker_viewer_app::DockerViewerApp;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
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

            // let mut containers = vec![];
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

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "dockerrs",
        options,
        Box::new(|_cc| {
            Box::new(DockerViewerApp {
                receiver,
                containers: HashMap::new(),
                selected_container: None,
            })
        }),
    )
    .unwrap();
}
