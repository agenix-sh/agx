pub mod agq_client;
pub mod cli;
pub mod executor;
pub mod input;
pub mod logging;
pub mod plan;
pub mod plan_buffer;
pub mod planner;
pub mod registry;

use serde_json::json;

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

    let command = config
        .command
        .take()
        .ok_or_else(|| "a command is required. Run `agx --help` for usage.".to_string())?;

    logging::set_debug(config.debug);

    if config.debug {
        logging::info("debug logging enabled");
    }

    match command {
        cli::Command::Plan(plan_command) => handle_plan_command(plan_command),
    }
}

fn handle_plan_command(command: cli::PlanCommand) -> Result<(), String> {
    enforce_instruction_limit(&command)?;

    let storage = plan_buffer::PlanStorage::from_env();

    match command {
        cli::PlanCommand::New => {
            storage.reset()?;

            print_json(json!({
                "status": "ok",
                "plan_path": storage.path().display().to_string(),
                "plan_steps": 0
            }));
        }
        cli::PlanCommand::Preview => {
            let plan = storage.load()?;
            print_json(json!({
                "status": "ok",
                "plan": plan
            }));
        }
        cli::PlanCommand::Submit => {
            let plan = storage.load()?;

            logging::info(&format!(
                "PLAN submit request with {} step(s)",
                plan.plan.len()
            ));

            let plan_json = serde_json::to_string(&plan)
                .map_err(|error| format!("failed to serialize plan for submission: {error}"))?;

            let agq_config = agq_client::AgqConfig::from_env();
            let client = agq_client::AgqClient::new(agq_config);

            match client.submit_plan(&plan_json) {
                Ok(submission) => {
                    let metadata = plan_buffer::PlanMetadata {
                        job_id: submission.job_id.clone(),
                        submitted_at: chrono::DateTime::<chrono::Utc>::from(
                            submission.submitted_at,
                        )
                        .to_rfc3339(),
                    };
                    storage.save_submission_metadata(&metadata)?;

                    print_json(json!({
                        "status": "ok",
                        "job_id": submission.job_id,
                        "plan_path": storage.path().display().to_string()
                    }));
                }
                Err(error) => {
                    return Err(format!("PLAN submit failed: {error}"));
                }
            }
        }
        cli::PlanCommand::Add { instruction } => {
            let input = collect_planner_input()?;
            logging::info(&format!(
                "instruction: {}; bytes: {}; lines: {}; binary: {}",
                instruction, input.bytes, input.lines, input.is_probably_binary
            ));

            let registry = registry::ToolRegistry::new();
            logging::info(&format!(
                "available tools: {}",
                registry.describe_for_planner()
            ));

            let planner_config = planner::PlannerConfig::from_env();
            let planner = planner::Planner::new(planner_config);

            let plan_output = planner.plan(&instruction, &input, &registry)?;
            logging::info(&format!("planner raw output: {}", plan_output.raw_json));

            let parsed = plan_output.parse()?;
            let executable_plan = parsed.normalize_for_execution();
            let added_steps = executable_plan.plan.len();

            let mut buffer = storage.load()?;
            buffer.plan.extend(executable_plan.plan.into_iter());

            logging::info(&format!(
                "PLAN add appended {added_steps} step(s); buffer now has {} step(s)",
                buffer.plan.len()
            ));

            storage.save(&buffer)?;

            print_json(json!({
                "status": "ok",
                "added_steps": added_steps,
                "total_steps": buffer.plan.len(),
                "plan_path": storage.path().display().to_string()
            }));
        }
    }

    Ok(())
}

fn collect_planner_input() -> Result<input::InputSummary, String> {
    if input::InputCollector::stdin_is_terminal() {
        return Ok(input::InputSummary::empty());
    }

    input::InputCollector::collect().map_err(|error| format!("failed to read from STDIN: {error}"))
}

fn enforce_instruction_limit(command: &cli::PlanCommand) -> Result<(), String> {
    const MAX_INSTRUCTION_BYTES: usize = 8 * 1024;

    if let cli::PlanCommand::Add { instruction } = command {
        if instruction.len() > MAX_INSTRUCTION_BYTES {
            return Err(format!(
                "instruction is too long ({} bytes > {} allowed)",
                instruction.len(),
                MAX_INSTRUCTION_BYTES
            ));
        }
    }

    Ok(())
}

fn print_json(value: serde_json::Value) {
    match serde_json::to_string_pretty(&value) {
        Ok(json_text) => println!("{json_text}"),
        Err(error) => eprintln!("failed to serialize CLI output: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforce_instruction_limit_rejects_large() {
        let long_instruction = "x".repeat(9 * 1024);
        let command = cli::PlanCommand::Add {
            instruction: long_instruction,
        };

        let result = enforce_instruction_limit(&command);
        assert!(result.is_err());
    }
}
