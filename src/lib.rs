pub mod cli;
pub mod logging;
pub mod input;
pub mod plan;
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

    let planner_config = planner::PlannerConfig::from_env();
    let planner = planner::Planner::new(planner_config);

    match planner.plan(&config.instruction, &input) {
        Ok(plan_output) => {
            logging::info(&format!("plan: {}", plan_output.raw_json));

            match plan_output.parse() {
                Ok(parsed) => {
                    let commands = parsed
                        .plan
                        .iter()
                        .map(|step| step.cmd.as_str())
                        .collect::<Vec<_>>()
                        .join(" | ");

                    logging::info(&format!("parsed plan steps: {}", commands));
                }
                Err(error) => {
                    logging::info(&format!("plan parse error: {error}"));
                }
            }
        }
        Err(error) => {
            logging::info(&format!("planner error: {error}"));
        }
    }

    Ok(())
}
