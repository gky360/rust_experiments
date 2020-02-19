#![warn(clippy::all)]

// mod command_stdio;
// mod command_timeout;
// mod hyper_client;
// mod oauth;
mod parallel;

pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;

pub fn run() -> Result<()> {
    parallel::run()
}
