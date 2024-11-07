use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;

use bollard::secret::ContainerSummary;
use eframe::{egui, App};

use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use walkdir::WalkDir;

use crate::{
    docker_connection::DockerConnection,
    utils::{build_docker_image, run_docker_compose_up},
};

#[derive(PartialEq)]
pub enum AppView {
    Containers,
    Composes,
    Dockerfiles,
}

pub struct DockerViewerApp {
    receiver: mpsc::Receiver<HashMap<String, (ContainerSummary, String)>>,
    docker_connection: DockerConnection,
    containers: HashMap<String, (ContainerSummary, String)>,
    selected_container: Option<String>,
    selected_summary_field: String,
    compose_files: Vec<PathBuf>,
    selected_compose_for_preview: Option<PathBuf>,
    current_view: AppView,
    dockerfiles: Vec<PathBuf>,
    selected_dockerfile_for_preview: Option<PathBuf>,
    docker_build_name: String,
}

impl DockerViewerApp {
    pub fn new(
        receiver: mpsc::Receiver<HashMap<String, (ContainerSummary, String)>>,
        docker_connection: DockerConnection,
    ) -> Self {
        Self {
            receiver,
            docker_connection,
            containers: HashMap::new(),
            selected_container: None,
            selected_summary_field: "id".to_owned(),
            current_view: AppView::Containers,
            selected_compose_for_preview: None,
            compose_files: Vec::new(),
            dockerfiles: Vec::new(),
            selected_dockerfile_for_preview: None,
            docker_build_name: "add tag".to_owned(),
        }
    }
}

impl App for DockerViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.current_view == AppView::Containers, "Containers")
                    .clicked()
                {
                    self.current_view = AppView::Containers;
                }
                if ui
                    .selectable_label(self.current_view == AppView::Composes, "Composes")
                    .clicked()
                {
                    self.current_view = AppView::Composes;
                }
                if ui
                    .selectable_label(self.current_view == AppView::Dockerfiles, "Dockerfiles")
                    .clicked()
                {
                    self.current_view = AppView::Dockerfiles;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Remove All").clicked() {
                        self.docker_connection.remove_all_containers();
                    }
                    if ui.button("Kill All").clicked() {
                        self.docker_connection.kill_all_containers();
                    }
                    if ui.button("Stop All").clicked() {
                        self.docker_connection.stop_all_containers();
                    }
                });
            });

            match self.current_view {
                AppView::Containers => {
                    self.containers_appview(ui);
                }
                AppView::Composes => {
                    self.composes_appview(ui);
                }
                AppView::Dockerfiles => {
                    self.dockerfiles_appview(ui);
                }
            }
        });

        ctx.request_repaint();
        sleep(Duration::from_millis(50));
    }
}

impl DockerViewerApp {
    fn composes_appview(&mut self, ui: &mut egui::Ui) {
        // Path and Docker containers separation line
        ui.vertical(|ui| {
            for path in &self.compose_files {
                ui.separator();
                ui.horizontal(|ui| {
                    // Extract the last three folders from the path
                    let folders: Vec<_> = path.iter().rev().collect();
                    let display_path = folders
                        .iter()
                        .rev()
                        .map(|p| p.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/");
                    if ui
                        .selectable_label(
                            self.selected_compose_for_preview == Some(path.clone()),
                            display_path,
                        )
                        .clicked()
                    {
                        self.selected_compose_for_preview = Some(path.clone())
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.selected_compose_for_preview.as_ref() == Some(path) {
                            if ui.button("Run").clicked() {
                                if let Some(parent) = path.parent() {
                                    let parent_clone = parent.to_owned();
                                    run_docker_compose_up(&parent_clone);
                                } else {
                                    eprintln!(
                                        "Error: Cannot determine the parent directory for {:?}",
                                        path
                                    );
                                }
                            }
                        }
                    });
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

    fn containers_appview(&mut self, ui: &mut egui::Ui) {
        while let Ok(new_containers) = self.receiver.try_recv() {
            self.containers = new_containers;
        }

        let mut container_names: Vec<_> = self.containers.keys().cloned().collect();
        container_names.sort();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for name in container_names {
                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.selected_container.as_ref() == Some(&name),
                            name.clone(),
                        )
                        .clicked()
                    {
                        self.selected_container = Some(name.clone());
                    }
                    if let Some((summary, _)) = self.containers.get(&name) {
                        if let Some(status) = summary.status.clone() {
                            ui.label(format!("Status: {} | ", status));
                        }
                        if let Some(image_name) = summary.image.clone() {
                            ui.label(format!("Image: {}", image_name));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.selected_container.as_ref() == Some(&name) {
                            match self.get_container_summary(&name) {
                                Some(summary) => {
                                    if ui.button("Remove").clicked() {
                                        self.docker_connection.remove_container(summary.id.clone());
                                    }
                                    if ui.button("Kill").clicked() {
                                        self.docker_connection.kill_container(summary.id.clone());
                                    }
                                    if summary.state == Some("running".to_owned()) {
                                        if ui.button("Stop").clicked() {
                                            self.docker_connection
                                                .stop_container(summary.id.clone());
                                        }
                                    } else {
                                        if ui.button("Start").clicked() {
                                            self.docker_connection
                                                .start_container(summary.id.clone());
                                        }
                                    }
                                }
                                None => {
                                    ui.label("No summary available");
                                }
                            }
                        }
                    })
                });
            }
        });

        self.display_summary_and_logs(ui);
    }

    fn get_container_summary(&self, container_id: &str) -> Option<ContainerSummary> {
        if let Some((container, _)) = self.containers.get(container_id) {
            Some(container.clone())
        } else {
            None
        }
    }

    fn display_summary_and_logs(&mut self, ui: &mut egui::Ui) {
        let Some(name) = &self.selected_container else {
            return;
        };
        let Some((summary, logs)) = self.containers.get(name) else {
            return;
        };
        ui.group(|ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label("Summary:");

                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.selected_summary_field == "ID", "ID")
                            .clicked()
                        {
                            self.selected_summary_field = "ID".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Names", "Names")
                            .clicked()
                        {
                            self.selected_summary_field = "Names".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Image", "Image")
                            .clicked()
                        {
                            self.selected_summary_field = "Image".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Image ID", "Image ID")
                            .clicked()
                        {
                            self.selected_summary_field = "Image ID".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Command", "Command")
                            .clicked()
                        {
                            self.selected_summary_field = "Command".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Created", "Created")
                            .clicked()
                        {
                            self.selected_summary_field = "Created".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Ports", "Ports")
                            .clicked()
                        {
                            self.selected_summary_field = "Ports".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Size RW", "Size RW")
                            .clicked()
                        {
                            self.selected_summary_field = "Size RW".to_string();
                        }
                        if ui
                            .selectable_label(
                                self.selected_summary_field == "Size Root FS",
                                "Size Root FS",
                            )
                            .clicked()
                        {
                            self.selected_summary_field = "Size Root FS".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Labels", "Labels")
                            .clicked()
                        {
                            self.selected_summary_field = "Labels".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "State", "State")
                            .clicked()
                        {
                            self.selected_summary_field = "State".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Status", "Status")
                            .clicked()
                        {
                            self.selected_summary_field = "Status".to_string();
                        }
                        if ui
                            .selectable_label(
                                self.selected_summary_field == "Host Config",
                                "Host Config",
                            )
                            .clicked()
                        {
                            self.selected_summary_field = "Host Config".to_string();
                        }
                        if ui
                            .selectable_label(
                                self.selected_summary_field == "Network Settings",
                                "Network Settings",
                            )
                            .clicked()
                        {
                            self.selected_summary_field = "Network Settings".to_string();
                        }
                        if ui
                            .selectable_label(self.selected_summary_field == "Mounts", "Mounts")
                            .clicked()
                        {
                            self.selected_summary_field = "Mounts".to_string();
                        }
                    });

                    match self.selected_summary_field.as_str() {
                        "ID" => {
                            ui.monospace(format!("{:?}", summary.id.clone().unwrap_or_default()))
                        }
                        "Names" => {
                            ui.monospace(format!("{:?}", summary.names.clone().unwrap_or_default()))
                        }
                        "Image" => {
                            ui.monospace(format!("{:?}", summary.image.clone().unwrap_or_default()))
                        }
                        "Image ID" => ui.monospace(format!(
                            "{:?}",
                            summary.image_id.clone().unwrap_or_default()
                        )),
                        "Command" => ui.monospace(format!(
                            "{:?}",
                            summary.command.clone().unwrap_or_default()
                        )),
                        "Created" => ui.monospace(format!(
                            "{:?}",
                            summary.created.clone().unwrap_or_default()
                        )),
                        "Ports" => {
                            ui.monospace(format!("{:?}", summary.ports.clone().unwrap_or_default()))
                        }
                        "Size RW" => ui.monospace(format!(
                            "{:?}",
                            summary.size_rw.clone().unwrap_or_default()
                        )),
                        "Size Root FS" => ui.monospace(format!(
                            "{:?}",
                            summary.size_root_fs.clone().unwrap_or_default()
                        )),
                        "Labels" => ui
                            .monospace(format!("{:?}", summary.labels.clone().unwrap_or_default())),
                        "State" => {
                            ui.monospace(format!("{:?}", summary.state.clone().unwrap_or_default()))
                        }
                        "Status" => ui
                            .monospace(format!("{:?}", summary.status.clone().unwrap_or_default())),
                        "Host Config" => ui.monospace(format!(
                            "{:?}",
                            summary.host_config.clone().unwrap_or_default().network_mode
                        )),
                        "Network Settings" => ui.monospace(format!(
                            "{:?}",
                            summary.network_settings.clone().unwrap_or_default()
                        )),
                        "Mounts" => ui
                            .monospace(format!("{:?}", summary.mounts.clone().unwrap_or_default())),
                        _ => ui.monospace(""),
                    };

                    ui.separator();
                    ui.label(format!("Logs: \n {}", logs));
                });
        });
    }

    fn dockerfiles_appview(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            for dockerfile in &self.dockerfiles {
                ui.separator();
                ui.horizontal(|ui| {
                    let display_path = dockerfile.to_string_lossy();
                    if ui
                        .selectable_label(
                            self.selected_dockerfile_for_preview == Some(dockerfile.clone()),
                            display_path,
                        )
                        .clicked()
                    {
                        self.selected_dockerfile_for_preview = Some(dockerfile.clone())
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.selected_dockerfile_for_preview.as_ref() == Some(dockerfile) {
                            // Add an input field for the image name

                            // Button to build the image
                            if ui.button("Build").clicked() {
                                // Check if an image name has been provided
                                if self.docker_build_name.is_empty() {
                                    eprintln!("Error: Please provide a name for the Docker image.");
                                } else if let Some(parent) = dockerfile.parent() {
                                    let parent_clone = parent.to_owned();
                                    let image_name_clone = self.docker_build_name.clone();
                                    build_docker_image(&parent_clone, &image_name_clone);
                                } else {
                                    eprintln!(
                                        "Error: Cannot determine the parent directory for {:?}",
                                        dockerfile
                                    );
                                }
                            }
                            ui.text_edit_singleline(&mut self.docker_build_name);
                        }
                    });
                });
            }
        });

        if let Some(selected_dockerfile) = &self.selected_dockerfile_for_preview {
            if let Ok(file_content) = std::fs::read_to_string(selected_dockerfile) {
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

    pub fn load_dockerfiles(&mut self, directory: &Path) {
        println!("Loading dockerfiles");
        let walker = WalkDir::new(directory).into_iter();
        self.dockerfiles = walker
            .filter_map(|entry| {
                match entry {
                    Ok(entry) if entry.path().is_file() => {
                        let file_name = entry.file_name().to_str();
                        if file_name == Some("Dockerfile") {
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
                        eprintln!("Error walking directory: {}", e);
                        None
                    }
                }
            })
            .collect();
    }

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
