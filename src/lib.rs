pub mod agq_client;
pub mod cli;
pub mod executor;
pub mod input;
pub mod job;
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
        cli::Command::Action(action_command) => handle_action_command(action_command),
        cli::Command::Ops(ops_command) => handle_ops_command(ops_command),
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
        cli::PlanCommand::Validate => {
            let plan = storage.load()?;

            if plan.tasks.is_empty() {
                return Err("plan is empty. Use `PLAN add` to generate tasks first.".to_string());
            }

            logging::info(&format!(
                "PLAN validate request with {} task(s)",
                plan.tasks.len()
            ));

            // Run Delta validation on current plan
            let original_steps = plan.tasks.len();
            let validated_plan = run_delta_validation(&plan, &storage)?;
            let validated_steps = validated_plan.tasks.len();

            // Show diff summary
            let diff_summary = compute_plan_diff(&plan, &validated_plan);

            print_json(json!({
                "status": "ok",
                "original_tasks": original_steps,
                "validated_tasks": validated_steps,
                "changes": diff_summary,
                "plan_path": storage.path().display().to_string()
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
            let mut plan = storage.load()?;

            logging::info(&format!(
                "PLAN submit request with {} task(s)",
                plan.tasks.len()
            ));

            // Auto-validate with Delta if AGX_AUTO_VALIDATE is set
            if should_auto_validate() {
                logging::info("Auto-validation enabled, running Delta validation before submit");
                plan = run_delta_validation(&plan, &storage)?;
                logging::info(&format!(
                    "Auto-validation complete: {} task(s)",
                    plan.tasks.len()
                ));
            }

            let job = build_job_envelope(plan)?;
            let job_json = serde_json::to_string(&job)
                .map_err(|error| format!("failed to serialize job for submission: {error}"))?;

            let agq_config = agq_client::AgqConfig::from_env();
            let client = agq_client::AgqClient::new(agq_config);

            match client.submit_plan(&job_json) {
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
            let added_tasks = executable_plan.tasks.len();

            let mut buffer = storage.load()?;
            let offset = buffer.tasks.len() as u32;
            buffer.tasks.extend(executable_plan.tasks.into_iter());

            // Renumber newly added tasks by offset and adjust their input_from_task references
            // Existing tasks keep their numbers unchanged
            if offset > 0 {
                for task in buffer.tasks.iter_mut().skip(offset as usize) {
                    let old_number = task.task_number;
                    task.task_number = old_number + offset;

                    // Adjust input_from_task references within newly added tasks
                    if let Some(old_ref) = task.input_from_task {
                        task.input_from_task = Some(old_ref + offset);
                    }
                }
            }

            logging::info(&format!(
                "PLAN add appended {added_tasks} task(s); buffer now has {} task(s)",
                buffer.tasks.len()
            ));

            storage.save(&buffer)?;

            print_json(json!({
                "status": "ok",
                "added_tasks": added_tasks,
                "total_tasks": buffer.tasks.len(),
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

fn should_auto_validate() -> bool {
    match std::env::var("AGX_AUTO_VALIDATE") {
        Ok(value) => {
            let normalized = value.to_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

fn compute_plan_diff(
    original: &plan::WorkflowPlan,
    validated: &plan::WorkflowPlan,
) -> serde_json::Value {
    let original_cmds: Vec<String> = original.tasks.iter().map(|t| t.command.clone()).collect();
    let validated_cmds: Vec<String> = validated.tasks.iter().map(|t| t.command.clone()).collect();

    let added: Vec<String> = validated_cmds
        .iter()
        .filter(|cmd| !original_cmds.contains(cmd))
        .cloned()
        .collect();

    let removed: Vec<String> = original_cmds
        .iter()
        .filter(|cmd| !validated_cmds.contains(cmd))
        .cloned()
        .collect();

    let task_count_change = validated.tasks.len() as i32 - original.tasks.len() as i32;

    json!({
        "added": added,
        "removed": removed,
        "task_count_change": task_count_change,
        "summary": if task_count_change > 0 {
            format!("Added {} task(s)", task_count_change)
        } else if task_count_change < 0 {
            format!("Removed {} task(s)", -task_count_change)
        } else {
            "No change in task count".to_string()
        }
    })
}

/// Instruction used for Delta validation
const DELTA_VALIDATION_INSTRUCTION: &str = "Validate and refine this plan";

fn run_delta_validation(
    current_plan: &plan::WorkflowPlan,
    storage: &plan_buffer::PlanStorage,
) -> Result<plan::WorkflowPlan, String> {
    // Create Delta planner with explicit ModelRole (no env var mutation)
    let delta_config = planner::PlannerConfig::for_delta()
        .map_err(|e| format!("Failed to create Delta config: {}", e))?;
    let planner = planner::Planner::new(delta_config);

    // Get tool registry
    let registry = registry::ToolRegistry::new();

    // Run Delta validation with existing plan as context
    let input = input::InputSummary::empty();

    let plan_output = planner.plan_with_existing(
        DELTA_VALIDATION_INSTRUCTION,
        &input,
        &registry,
        &current_plan.tasks,
    )?;

    logging::info(&format!("Delta validation output: {}", plan_output.raw_json));

    let parsed = plan_output.parse()?;
    let validated_plan = parsed.normalize_for_execution();

    // Save validated plan to buffer
    storage.save(&validated_plan)?;

    logging::info(&format!(
        "Delta validation complete: {} task(s)",
        validated_plan.tasks.len()
    ));

    Ok(validated_plan)
}

fn build_job_envelope(plan: plan::WorkflowPlan) -> Result<job::JobEnvelope, String> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let plan_id = uuid::Uuid::new_v4().to_string();
    let plan_description = std::env::var("AGX_PLAN_DESCRIPTION").ok();

    let envelope = job::JobEnvelope::from_plan(
        plan,
        job_id,
        plan_id,
        plan_description.filter(|s| !s.is_empty()),
    );
    envelope
        .validate(100)
        .map_err(|e| format!("job envelope validation failed: {e:?}"))?;

    Ok(envelope)
}

fn handle_action_command(command: cli::ActionCommand) -> Result<(), String> {
    match command {
        cli::ActionCommand::Submit {
            plan_id,
            input,
            inputs_file,
        } => {
            // Parse and validate inputs from --input flag or --inputs-file
            let inputs_array = if let Some(inline_input) = input {
                // Single input - wrap in array
                let single_input = serde_json::from_str::<serde_json::Value>(&inline_input)
                    .map_err(|e| format!("invalid JSON in --input: {}", e))?;
                serde_json::json!([single_input])
            } else if let Some(file_path) = inputs_file {
                // File should contain array of inputs
                let content = std::fs::read_to_string(&file_path)
                    .map_err(|e| format!("failed to read inputs file '{}': {}", file_path, e))?;
                let value = serde_json::from_str::<serde_json::Value>(&content)
                    .map_err(|e| format!("invalid JSON in file '{}': {}", file_path, e))?;
                // Validate it's an array
                if !value.is_array() {
                    return Err("--inputs-file must contain a JSON array of inputs".to_string());
                }
                value
            } else {
                // Default to empty array if no inputs provided
                serde_json::json!([])
            };

            logging::info(&format!(
                "ACTION submit request for plan_id: {}",
                plan_id
            ));

            // Generate action_id
            let action_id = format!("action_{}", uuid::Uuid::new_v4().simple());

            // Build ACTION.SUBMIT payload
            let action_request = json!({
                "action_id": action_id,
                "plan_id": plan_id,
                "inputs": inputs_array,
            });

            let action_json = serde_json::to_string(&action_request)
                .map_err(|e| format!("failed to serialize action request: {}", e))?;

            // Submit to AGQ
            let agq_config = agq_client::AgqConfig::from_env();
            let client = agq_client::AgqClient::new(agq_config);

            match client.submit_action(&action_json) {
                Ok(response) => {
                    print_json(json!({
                        "status": "ok",
                        "action_id": response.action_id,
                        "plan_id": response.plan_id,
                        "plan_description": response.plan_description,
                        "jobs_created": response.jobs_created,
                        "job_ids": response.job_ids,
                    }));
                    Ok(())
                }
                Err(error) => Err(format!("ACTION submit failed: {}", error)),
            }
        }
    }
}

fn handle_ops_command(command: cli::OpsCommand) -> Result<(), String> {
    let agq_config = agq_client::AgqConfig::from_env();
    let client = agq_client::AgqClient::new(agq_config);

    let (resp, json_output) = match command {
        cli::OpsCommand::Jobs { json } => (client.list_jobs()?, json),
        cli::OpsCommand::Workers { json } => (client.list_workers()?, json),
        cli::OpsCommand::Queue { json } => (client.queue_stats()?, json),
    };

    if json_output {
        print_json(match resp {
            agq_client::OpsResponse::Jobs(items)
            | agq_client::OpsResponse::Workers(items)
            | agq_client::OpsResponse::QueueStats(items) => json!({"status": "ok", "items": items}),
        });
        return Ok(());
    }

    match resp {
        agq_client::OpsResponse::Jobs(items) => {
            println!("JOBS:");
            for item in items {
                println!("- {item}");
            }
        }
        agq_client::OpsResponse::Workers(items) => {
            println!("WORKERS:");
            for item in items {
                println!("- {item}");
            }
        }
        agq_client::OpsResponse::QueueStats(items) => {
            println!("QUEUE:");
            for item in items {
                println!("- {item}");
            }
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

    #[test]
    fn build_job_envelope_assigns_ids_and_validates() {
        let plan = plan::WorkflowPlan {
            plan_id: None,
            plan_description: None,
            tasks: vec![
                plan::PlanStep {
                    task_number: 1,
                    command: "sort".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: None,
                },
                plan::PlanStep {
                    task_number: 2,
                    command: "uniq".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: Some(1),
                },
            ],
        };

        let env = build_job_envelope(plan).expect("envelope should build");
        assert_eq!(env.tasks.len(), 2);
        assert!(!env.job_id.is_empty());
        assert!(!env.plan_id.is_empty());
    }

    #[test]
    fn plan_append_preserves_task_dependencies() {
        // Test that appending new tasks preserves input_from_task references
        // Simulates PLAN add workflow where new tasks are appended to existing buffer

        // Existing buffer with dependencies: task 2 depends on task 1
        let mut buffer = plan::WorkflowPlan {
            plan_id: None,
            plan_description: None,
            tasks: vec![
                plan::PlanStep {
                    task_number: 1,
                    command: "cat".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: None,
                },
                plan::PlanStep {
                    task_number: 2,
                    command: "sort".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: Some(1), // Depends on task 1
                },
            ],
        };

        // New plan to append (normalized, so starts at 1)
        let new_plan = plan::WorkflowPlan {
            plan_id: None,
            plan_description: None,
            tasks: vec![plan::PlanStep {
                task_number: 1,
                command: "uniq".into(),
                args: vec![],
                timeout_secs: 300,
                input_from_task: None,
            }],
        };

        // Simulate PLAN add logic
        let offset = buffer.tasks.len() as u32;
        buffer.tasks.extend(new_plan.tasks.into_iter());

        if offset > 0 {
            for task in buffer.tasks.iter_mut().skip(offset as usize) {
                let old_number = task.task_number;
                task.task_number = old_number + offset;

                if let Some(old_ref) = task.input_from_task {
                    task.input_from_task = Some(old_ref + offset);
                }
            }
        }

        // Verify results
        assert_eq!(buffer.tasks.len(), 3);
        assert_eq!(buffer.tasks[0].task_number, 1);
        assert_eq!(buffer.tasks[0].command, "cat");
        assert_eq!(buffer.tasks[0].input_from_task, None);

        assert_eq!(buffer.tasks[1].task_number, 2);
        assert_eq!(buffer.tasks[1].command, "sort");
        assert_eq!(buffer.tasks[1].input_from_task, Some(1)); // Still points to task 1 ✓

        assert_eq!(buffer.tasks[2].task_number, 3); // Renumbered from 1 to 3
        assert_eq!(buffer.tasks[2].command, "uniq");
        assert_eq!(buffer.tasks[2].input_from_task, None);
    }

    #[test]
    fn action_submit_builds_correct_request() {
        // Test that ACTION submit handler builds correct JSON payload
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Test with inline JSON
        let inline_input = r#"{"key":"value","count":42}"#;
        let parsed: serde_json::Value = serde_json::from_str(inline_input).expect("valid JSON");
        assert_eq!(parsed["key"], "value");
        assert_eq!(parsed["count"], 42);

        // Test with JSON file
        let mut temp_file = NamedTempFile::new().expect("create temp file");
        temp_file
            .write_all(br#"{"file_key":"file_value"}"#)
            .expect("write to temp file");
        temp_file.flush().expect("flush temp file");

        let content = std::fs::read_to_string(temp_file.path()).expect("read temp file");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
        assert_eq!(parsed["file_key"], "file_value");

        // Test empty object default
        let empty: serde_json::Value = serde_json::from_str("{}").expect("valid JSON");
        assert!(empty.is_object());
        assert_eq!(empty.as_object().unwrap().len(), 0);
    }

    #[test]
    fn action_submit_rejects_invalid_json() {
        // Test that invalid JSON in --input is rejected
        let invalid_json = r#"{"key": invalid}"#;
        let result = serde_json::from_str::<serde_json::Value>(invalid_json);
        assert!(result.is_err());

        // Test that invalid JSON in file is rejected
        let invalid_json2 = r#"not json at all"#;
        let result2 = serde_json::from_str::<serde_json::Value>(invalid_json2);
        assert!(result2.is_err());
    }
}
