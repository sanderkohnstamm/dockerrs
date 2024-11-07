use tokio::process::Command;

use std::path::Path;

pub fn run_docker_compose_up(directory: &Path) {
    println!("Running 'docker compose up' in {:?}", directory);
    let directory = directory.to_path_buf();
    tokio::spawn(async move {
        match Command::new("docker")
            .arg("compose")
            .arg("up")
            .arg("-d") // Run in detached mode
            .current_dir(directory.clone())
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
    });
}

pub fn build_docker_image(dockerfile: &Path, image_name: &str) {
    let dockerfile = dockerfile.to_path_buf();
    let image_name = image_name.to_string();

    tokio::spawn(async move {
        println!(
            "Building Docker image from {:?}, named {:?}",
            dockerfile, image_name
        );

        let output = Command::new("docker")
            .arg("build")
            .arg("-t")
            .arg(&image_name)
            .arg(&dockerfile)
            .output()
            .await
            .expect("Failed to execute process");

        println!("status: {}", output.status);
        println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    });
}
