use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;

use bollard::container::{KillContainerOptions, RemoveContainerOptions};
use bollard::secret::ContainerSummary;
use bollard::Docker;
use eframe::{egui, App};
use tokio::sync::mpsc;

pub struct DockerViewerApp {
    pub receiver: mpsc::Receiver<HashMap<String, (ContainerSummary, String)>>,
    pub containers: HashMap<String, (ContainerSummary, String)>,
    pub selected_container: Option<String>,
}

impl App for DockerViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Containers");
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

            while let Ok(new_containers) = self.receiver.try_recv() {
                self.containers = new_containers;
            }

            let mut container_names: Vec<_> = self.containers.keys().collect();
            container_names.sort();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for name in container_names {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.selected_container.as_ref() == Some(name), name)
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
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.selected_container.as_ref() == Some(name) {
                                if ui.button("Remove").clicked() {
                                    if let Some((summary, _logs)) = self.containers.get(name) {
                                        let summary_clone = summary.clone();
                                        tokio::spawn(async move {
                                            remove_container(&summary_clone).await
                                        });
                                    }
                                }
                                if ui.button("Kill").clicked() {
                                    if let Some((summary, _logs)) = self.containers.get(name) {
                                        let summary_clone = summary.clone();
                                        tokio::spawn(async move {
                                            kill_container(&summary_clone).await
                                        });
                                    }
                                }
                            }
                        })
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
        });
        ctx.request_repaint();
        sleep(Duration::from_millis(50));
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
