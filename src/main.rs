use std::process::Stdio;
use std::{env, panic};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::Command;

macro_rules! magenta {
    ($input:expr) => {{
        format!("\x1b[35m{}\x1b[0m", $input)
    }};
}

macro_rules! magenta2 {
    ($input:expr) => {{
        format!("\x1b[35m{:?}\x1b[0m", $input)
    }};
}

macro_rules! yellow {
    ($input:expr) => {{
        format!("\x1b[33m{}\x1b[0m", $input)
    }};
}

macro_rules! red2 {
    ($input:expr) => {{
        format!("\x1b[31m{:?}\x1b[0m", $input)
    }};
}

#[tokio::main]
async fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let shim_config = read_shim_file().await;

    let mut child = match shim_config.log {
        Some(log_file) => {
            let mut child = Command::new(&shim_config.path)
                .args(args)
                .args(shim_config.args)
                .stdin(Stdio::inherit())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap_or_else(|e| {
                    panic!(
                        "Unable to spawn program: {}\n{}",
                        magenta!(&shim_config.path),
                        red2!(e)
                    )
                });

            let mut stdout_log_file = File::create(format!("{}.stdout.log", &log_file))
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "Unable to open log file: {}{}\n{}",
                        magenta!(&log_file),
                        magenta!(".stdout.log"),
                        red2!(e)
                    )
                });
            let mut stderr_log_file = File::create(format!("{}.stderr.log", &log_file))
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "Unable to open log file: {}{}\n{}",
                        magenta!(&log_file),
                        magenta!("stderr.log"),
                        red2!(e)
                    )
                });
            let stdout = child.stdout.take().expect("Failed to open stdout");
            let stderr = child.stderr.take().expect("Failed to open stderr");
            let stdout_reader = tokio::io::BufReader::new(stdout);
            let stderr_reader = tokio::io::BufReader::new(stderr);
            let mut stdout_writer = tokio::io::stdout();
            let mut stderr_writer = tokio::io::stderr();
            let stdout_task =
                copy_and_print(stdout_reader, &mut stdout_writer, &mut stdout_log_file);
            let stderr_task =
                copy_and_print(stderr_reader, &mut stderr_writer, &mut stderr_log_file);
            tokio::try_join!(stdout_task, stderr_task)
                .expect("Unable to join with stdout_task and/or stderr_task");
            child
        }
        None => {
            // No log file, just passthrough everything
            let child = Command::new(&shim_config.path)
                .args(args)
                .args(shim_config.args)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap_or_else(|e| {
                    panic!(
                        "Unable to spawn program: {}\n{}",
                        magenta!(&shim_config.path),
                        red2!(e)
                    )
                });
            child
        }
    };

    let result = child.wait().await.expect("Unable to wait for childer");
    if !result.success() {
        panic!("Command failed with status: {}", magenta!(result));
    }
}

async fn copy_and_print(
    mut reader: tokio::io::BufReader<impl AsyncReadExt + Unpin>,
    mut writer: impl AsyncWriteExt + Unpin,
    log_file: &mut File,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; 1024];
    loop {
        match reader.read(&mut buffer).await? {
            0 => break,
            n => {
                writer.write_all(&buffer[..n]).await?;
                // Flush writer so stdout and stderr will be visible rightway
                writer.flush().await?;
                log_file.write_all(&buffer[..n]).await?;
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct ShimConfig {
    path: String,
    args: Option<String>,
    log: Option<String>,
}
async fn read_shim_file() -> ShimConfig {
    let exe_path = env::current_exe().expect("exe_path");
    let mut shim_path = exe_path.clone();
    shim_path.set_extension("shim");

    let file = File::open(&shim_path).await.unwrap_or_else(|e| {
        panic!(
            "Unable to open the {} file.\n{}",
            magenta2!(&shim_path),
            red2!(e)
        )
    });
    let reader = BufReader::new(file);
    let mut config = ShimConfig {
        path: String::new(),
        args: None,
        log: None,
    };
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await.unwrap_or_else(|e| {
        panic!(
            "Unable to read lines from {}\n{}",
            magenta2!(&shim_path),
            red2!(e)
        )
    }) {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "path" => config.path = value.to_string(),
                "args" => config.args = Some(value.to_string()),
                "log" => config.log = Some(value.to_string()),
                _ => {}
            }
        }
    }
    if config.path.is_empty() {
        panic!(
            "Unable to find {} in the {} file.",
            yellow!("path = \"path/to/program\""),
            magenta2!(&shim_path)
        );
    }
    config
}
