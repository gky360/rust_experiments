use std::io::{self, BufRead as _};
use std::process::{Command, Stdio};

use crate::Result;

static SCRIPT: &str = r#"
sleep 1
echo hello
sleep 1
echo hello
"#;

pub fn run() -> Result<()> {
    let mut bash_handle = Command::new("bash")
        .args(&["-c", SCRIPT])
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = bash_handle.stdout.take().unwrap();
    let mut output = io::BufReader::new(stdout);

    let mut buf = String::new();
    while output.read_line(&mut buf)? > 0 {
        eprint!("{}", buf);
        buf.clear();
    }

    Ok(())
}
