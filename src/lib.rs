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

            print_json(json!({
                "status": "pending",
                "message": "AGQ submission not yet implemented. See issue #31.",
                "plan": plan
            }));
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

fn print_json(value: serde_json::Value) {
    match serde_json::to_string_pretty(&value) {
        Ok(json_text) => println!("{json_text}"),
        Err(error) => eprintln!("failed to serialize CLI output: {error}"),
    }
}
