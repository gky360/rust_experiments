use std::io::Write as _;
use std::process::{Command, Stdio};

use crate::Result;

static INPUT: &str = r#"
This is a string.
"#;

static SCRIPT: &str = r#"
sleep 1
echo hello
echo hello 1>&2
sleep 1
cat
sleep 1
echo hello
"#;

pub fn run() -> Result<()> {
    let mut child = Command::new("bash")
        .args(&["-c", SCRIPT])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(INPUT.as_bytes())?;
    eprintln!("{:?}", child.wait_with_output()?);

    Ok(())
}
