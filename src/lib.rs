pub mod cli;
pub mod logging;
pub mod input;
pub mod planner;
pub mod registry;
pub mod executor;

pub fn run() -> Result<(), String> {
    let config = cli::CliConfig::from_env()?;

    let input = input::InputCollector::collect()
        .map_err(|error| format!("failed to read from STDIN: {error}"))?;

    logging::info(&format!(
        "instruction: {}; bytes: {}; lines: {}; binary: {}",
        config.instruction, input.bytes, input.lines, input.is_probably_binary
    ));

    Ok(())
}
