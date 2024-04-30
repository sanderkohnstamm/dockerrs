use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;

use bollard::container::{KillContainerOptions, RemoveContainerOptions};
use bollard::secret::ContainerSummary;
use bollard::Docker;
use eframe::{egui, App};
use tokio::process::Command;

use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use walkdir::WalkDir;

pub enum AppView {
    Containers,
    Composes,
}

pub struct DockerViewerApp {
    pub receiver: mpsc::Receiver<HashMap<String, (ContainerSummary, String)>>,
    pub containers: HashMap<String, (ContainerSummary, String)>,
    pub selected_container: Option<String>,
    pub compose_files: Vec<PathBuf>,
    pub selected_compose_for_preview: Option<PathBuf>,
    pub current_view: AppView,
}

impl App for DockerViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Containers").clicked() {
                    self.current_view = AppView::Containers;
                }
                if ui.button("Composes").clicked() {
                    self.current_view = AppView::Composes;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Remove All").clicked() {
                        let all_summaries: Vec<ContainerSummary> = self
                            .containers
                            .values()
                            .cloned()
                            .into_iter()
                            .map(|a| a.0)
                            .collect();
                        tokio::spawn(async move { remove_containers(all_summaries).await });
                    }
                    if ui.button("Kill All").clicked() {
                        let all_summaries: Vec<ContainerSummary> = self
                            .containers
                            .values()
                            .cloned()
                            .into_iter()
                            .map(|a| a.0)
                            .collect();
                        tokio::spawn(async move { kill_containers(all_summaries).await });
                    }
                });
            });

            match self.current_view {
                AppView::Containers => {
                    while let Ok(new_containers) = self.receiver.try_recv() {
                        self.containers = new_containers;
                    }

                    let mut container_names: Vec<_> = self.containers.keys().collect();
                    container_names.sort();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for name in container_names {
                            ui.separator();

                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        self.selected_container.as_ref() == Some(name),
                                        name,
                                    )
                                    .clicked()
                                {
                                    self.selected_container = Some(name.clone());
                                }
                                if let Some((summary, _)) = self.containers.get(name) {
                                    if let Some(status) = summary.status.clone() {
                                        ui.label(format!("Status: {} | ", status));
                                    }

                                    if let Some(image_name) = summary.image.clone() {
                                        ui.label(format!("Image: {}", image_name));
                                    }
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if self.selected_container.as_ref() == Some(name) {
                                            if ui.button("Remove").clicked() {
                                                if let Some((summary, _logs)) =
                                                    self.containers.get(name)
                                                {
                                                    let summary_clone = summary.clone();
                                                    tokio::spawn(async move {
                                                        remove_container(&summary_clone).await
                                                    });
                                                }
                                            }
                                            if ui.button("Kill").clicked() {
                                                if let Some((summary, _logs)) =
                                                    self.containers.get(name)
                                                {
                                                    let summary_clone = summary.clone();
                                                    tokio::spawn(async move {
                                                        kill_container(&summary_clone).await
                                                    });
                                                }
                                            }
                                        }
                                    },
                                )
                            });
                        }
                    });

                    if let Some(name) = &self.selected_container {
                        if let Some((_summary, logs)) = self.containers.get(name) {
                            ui.group(|ui| {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.label(logs);
                                    });
                            });
                        }
                    }

                    // Path and Docker containers separation line
                }
                AppView::Composes => {
                    // Path and Docker containers separation line
                    ui.vertical(|ui| {
                        for path in &self.compose_files {
                            ui.separator();
                            ui.horizontal(|ui| {
                                // Extract the last three folders from the path
                                let folders: Vec<_> = path.iter().rev().take(3).collect();
                                let display_path = folders.iter().rev().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("/");
                                if ui.selectable_label(
                                    self.selected_compose_for_preview == Some(path.clone()),
                                    display_path,
                                )
                                .clicked(){
                                    self.selected_compose_for_preview = Some(path.clone())
                                }
                      
                    
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Run").clicked() {
                                            if let Some(parent) = path.parent() {
                                                let parent_clone = parent.to_owned();
                                                tokio::spawn(async move {
                                                    run_docker_compose_up(&parent_clone).await;
                                                });
                                            } else {
                                                eprintln!(
                                                    "Error: Cannot determine the parent directory for {:?}",
                                                    path
                                                );
                                            }
                                        }
                                    },
                                );
                            });
                        }
                    });
                    // Display compose preview if a file is selected
                    if let Some(selected_compose) = &self.selected_compose_for_preview {
                        if let Ok(file_content) = std::fs::read_to_string(selected_compose) {
                            ui.group(|ui| {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.label(file_content);
                                    });
                            });
                        }
                    }
                }
            }
        });

        ctx.request_repaint();
        sleep(Duration::from_millis(50));
    }
}

impl DockerViewerApp {
    pub fn load_compose_files(&mut self, directory: &Path) {
        println!("Loading compose files");
        let walker = WalkDir::new(directory).into_iter();
        self.compose_files = walker
            .filter_map(|entry| {
                match entry {
                    Ok(entry) if entry.path().is_file() => {
                        let file_name = entry.file_name().to_str();
                        if file_name == Some("docker_compose.yaml")
                            || file_name == Some("docker-compose.yaml")
                        {
                            // Resolve the path to an absolute path
                            let abs_path = entry.path().canonicalize();
                            match abs_path {
                                Ok(path) => {
                                    println!("File found: {:?}", path);
                                    Some(path)
                                }
                                Err(e) => {
                                    eprintln!("Error resolving path {:?}: {}", entry.path(), e);
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    }
                    Ok(_) => None,
                    Err(e) => {
                        eprintln!("Error reading directory entry: {}", e);
                        None
                    }
                }
            })
            .collect();
    }
}

async fn run_docker_compose_up(directory: &Path) {
    println!("Running 'docker compose up' in {:?}", directory);

    match Command::new("docker")
        .arg("compose")
        .arg("up")
        .arg("-d") // Run in detached mode
        .current_dir(directory)
        .status()
        .await
    {
        Ok(status) if status.success() => {
            println!("docker compose up executed successfully in {:?}", directory);
        }
        Ok(status) => {
            eprintln!(
                "docker compose up failed in {:?} with exit code {}",
                directory, status
            );
        }
        Err(e) => {
            eprintln!(
                "Failed to execute docker compose up in {:?}: {}",
                directory, e
            );
        }
    }
}

async fn kill_containers(containers: Vec<ContainerSummary>) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    for container in containers {
        _kill_container(&docker, &container).await;
    }
}
async fn remove_containers(containers: Vec<ContainerSummary>) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    for container in containers {
        _remove_container(&docker, &container).await;
    }
}

async fn kill_container(container: &ContainerSummary) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    _kill_container(&docker, container).await;
}

async fn _kill_container(docker: &Docker, container: &ContainerSummary) {
    let Some(container_id) = container.id.clone() else {
        return;
    };
    let kill_options = KillContainerOptions { signal: "SIGKILL" };
    if let Err(e) = docker
        .kill_container(&container_id, Some(kill_options))
        .await
    {
        eprintln!("Failed to kill container {}: {}", container_id, e);
    }
}

async fn remove_container(container: &ContainerSummary) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    _remove_container(&docker, container).await;
}

async fn _remove_container(docker: &Docker, container: &ContainerSummary) {
    let Some(container_id) = container.id.clone() else {
        return;
    };

    let remove_options = RemoveContainerOptions {
        force: true,
        ..Default::default()
    };
    if let Err(e) = docker
        .remove_container(&container_id, Some(remove_options))
        .await
    {
        eprintln!("Failed to remove container {}: {}", container_id, e);
    }
}
