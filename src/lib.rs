#![warn(clippy::all)]

// mod command_stdio;
mod command_timeout;

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;

pub fn run() -> Result<()> {
    command_timeout::run()
}
