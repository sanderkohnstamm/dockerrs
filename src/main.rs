pub mod docker_connection;
pub mod docker_viewer_app;
pub mod utils;

use bollard::secret::ContainerSummary;
use bollard::Docker;

use docker_connection::DockerConnection;
use docker_viewer_app::DockerViewerApp;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::mpsc::{self, Receiver};

#[tokio::main]
async fn main() {
    let (sender, receiver) = mpsc::channel(100);
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    let docker_connection = DockerConnection::new(docker, sender);
    run_app(receiver, docker_connection);
}

fn run_app(
    receiver: Receiver<HashMap<String, (ContainerSummary, String)>>,
    docker_connection: DockerConnection,
) {
    let options = eframe::NativeOptions::default();
    let mut app = DockerViewerApp::new(receiver, docker_connection);
    app.load_compose_files(Path::new("../"));
    app.load_dockerfiles(Path::new("../"));
    eframe::run_native("dockerrs", options, Box::new(|_cc| Box::new(app))).unwrap();
}
