pub mod cli;
pub mod logging;
pub mod input;
pub mod planner;
pub mod registry;
pub mod executor;

pub fn run() -> Result<(), String> {
    let config = cli::CliConfig::from_env()?;

    logging::info(&format!("instruction: {}", config.instruction));

    Ok(())
}

