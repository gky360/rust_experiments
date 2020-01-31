#![warn(clippy::all)]

mod command_stdio;

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;

pub fn run() -> Result<()> {
    command_stdio::run()
}
