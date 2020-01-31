use std::process::Stdio;

use anyhow::Context as _;
use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

use crate::Result;

static INPUT: &str = r#"This is a string.
"#;

static SCRIPT: &str = r#"
sleep 1
echo hello 1>&2
sleep 1
cat
sleep 1
echo hello
"#;

#[tokio::main]
pub async fn run() -> Result<()> {
    let mut child = Command::new("bash")
        .args(&["-c", SCRIPT])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("Command failed to start")?;
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(INPUT.as_bytes()).await?;
    let output = child
        .wait_with_output()
        .await
        .context("Command failed to run")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    eprintln!("output: ---\n{}", stdout);

    Ok(())
}
