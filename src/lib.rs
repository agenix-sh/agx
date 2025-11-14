pub mod cli;
pub mod logging;
pub mod input;
pub mod plan;
pub mod planner;
pub mod registry;
pub mod executor;

pub fn run() -> Result<(), String> {
    let mut config = cli::CliConfig::from_env()?;

    if config.show_version {
        cli::print_version();
    }

    if config.show_help {
        cli::print_help();
    }

    if config.show_version || config.show_help {
        return Ok(());
    }

    let instruction = config.instruction.take().ok_or_else(|| {
        "an instruction is required. Run `agx --help` or `agx -v` for usage information."
            .to_string()
    })?;

    logging::set_debug(config.debug);

    if config.debug {
        logging::info("debug logging enabled");
    }

    let input = input::InputCollector::collect()
        .map_err(|error| format!("failed to read from STDIN: {error}"))?;

    logging::info(&format!(
        "instruction: {}; bytes: {}; lines: {}; binary: {}",
        instruction, input.bytes, input.lines, input.is_probably_binary
    ));

    let registry = registry::ToolRegistry::new();

    let planner_config = planner::PlannerConfig::from_env();
    let planner = planner::Planner::new(planner_config);

    match planner.plan(&instruction, &input, &registry) {
        Ok(plan_output) => {
            logging::info(&format!("plan: {}", plan_output.raw_json));

            match plan_output.parse() {
                Ok(parsed) => {
                    let executable_plan = parsed.normalize_for_execution();

                    let commands = executable_plan
                        .plan
                        .iter()
                        .map(|step| step.cmd.as_str())
                        .collect::<Vec<_>>()
                        .join(" | ");

                    logging::info(&format!("parsed plan steps: {}", commands));

                    let executor = executor::Executor::new();

                    if let Err(error) = executor.execute(&executable_plan, &input, &registry) {
                        logging::info(&format!("executor error: {error}"));
                    }
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
