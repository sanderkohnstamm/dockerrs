use bollard::container::{KillContainerOptions, RemoveContainerOptions};
use bollard::secret::ContainerSummary;
use bollard::Docker;
use tokio::process::Command;

use std::path::Path;

pub async fn run_docker_compose_up(directory: &Path) {
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

pub async fn build_docker_image(dockerfile: &Path) {
    println!(
        "Building Docker image from {:?}, named {:?}",
        dockerfile,
        dockerfile.file_stem().unwrap()
    );

    let output = Command::new("docker")
        .arg("build")
        .arg("-t")
        // Use the file name as the image name
        .arg(dockerfile.file_stem().unwrap())
        .arg(dockerfile)
        .output()
        .await
        .expect("Failed to execute process");

    println!("status: {}", output.status);
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
}

pub async fn kill_containers(containers: Vec<ContainerSummary>) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    for container in containers {
        _kill_container(&docker, &container).await;
    }
}
pub async fn remove_containers(containers: Vec<ContainerSummary>) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    for container in containers {
        _remove_container(&docker, &container).await;
    }
}

pub async fn kill_container(container: &ContainerSummary) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    _kill_container(&docker, container).await;
}

pub async fn _kill_container(docker: &Docker, container: &ContainerSummary) {
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

pub async fn remove_container(container: &ContainerSummary) {
    let docker = Docker::connect_with_unix_defaults().expect("Failed to connect to Docker");
    _remove_container(&docker, container).await;
}

pub async fn _remove_container(docker: &Docker, container: &ContainerSummary) {
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
