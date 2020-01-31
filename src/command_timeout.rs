use std::process::{Output, Stdio};
use std::time::Duration;

use anyhow::Context as _;
use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;
use tokio::time::timeout;

use crate::Result;

static INPUT: &str = r#"This is a string.
"#;

static SCRIPT: &str = r#"
sleep 1
echo hello 1>&2
sleep 1
cat
sleep 1
echo hello 1>&2
"#;

async fn run_child(mut command: Command, input: &[u8]) -> Result<Output> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Command failed to start")?;
    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(input)
        .await
        .context("Could not write input to stdin")?;
    let output = child
        .wait_with_output()
        .await
        .context("Command failed to run")?;
    Ok(output)
}

#[tokio::main]
pub async fn run() -> Result<()> {
    let mut command = Command::new("bash");
    command.args(&["-c", SCRIPT]);
    let result = timeout(Duration::from_secs(2), run_child(command, INPUT.as_bytes())).await;
    eprintln!("{:?}", result);

    // let stdout = String::from_utf8_lossy(&output.stdout);
    // eprintln!("output: ---\n{}", stdout);

    Ok(())
}
