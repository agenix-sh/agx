// Version from Cargo.toml - automatically synchronized with releases
const DISPLAY_VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP_TEXT: &str = "\
AGX - Agentic planner CLI (Phase 1)\n\
\n\
Usage:\n\
    agx [OPTIONS]            Start interactive REPL mode (default).\n\
    agx [OPTIONS] PLAN <subcommand>\n\
    agx [OPTIONS] ACTION submit --plan-id <ID> [--input <json>] [--inputs-file <path>] [--json]\n\
    agx [OPTIONS] JOBS list [--json]\n\
    agx [OPTIONS] WORKERS list [--json]\n\
    agx [OPTIONS] QUEUE stats [--json]\n\
\n\
PLAN subcommands:\n\
    PLAN new                 Reset the persisted plan buffer.\n\
    PLAN add \"<instruction>\"  Append planner-generated steps. Reads STDIN when piped.\n\
    PLAN validate            Run Delta model validation on current plan.\n\
    PLAN preview             Pretty-print the current JSON plan buffer.\n\
    PLAN submit [--json]     Validate the plan and submit to AGQ.\n\
    PLAN list [--json]       List all stored plans from AGQ.\n\
    PLAN get <plan-id>       View details of a specific plan.\n\
\n\
ACTION subcommands:\n\
    ACTION submit            Execute a plan with data inputs.\n\
      --plan-id <ID>         Plan ID to execute (required, non-empty).\n\
      --input <json>         Inline JSON input data (mutually exclusive with --inputs-file).\n\
      --inputs-file <path>   Path to file containing JSON input data (mutually exclusive with --input).\n\
      --json                 Output result as JSON (default: human-readable).\n\
\n\
Ops commands:\n\
    JOBS list                List jobs from AGQ (add --json for machine output).\n\
    WORKERS list             List workers and capabilities (add --json for machine output).\n\
    QUEUE stats              Show queue statistics (add --json for machine output).\n\
\n\
Options:\n\
    -h, --help        Print this help text.\n\
    -v, --version     Show the version and this help output.\n\
    -d, --debug       Enable verbose logging to stderr.\n\
\n\
Environment variables:\n\
    AGX_PLAN_PATH       Override the plan buffer location (default: $TMPDIR/agx-plan.json).\n\
    AGX_BACKEND         Planner backend (ollama or candle).\n\
    AGX_MODEL_ROLE      Model role (echo or delta, default: echo).\n\
    AGX_AUTO_VALIDATE   Auto-run Delta validation before submit (true/false, default: false).\n\
    AGX_OLLAMA_MODEL    Ollama model to run when using the Ollama backend (default: phi3:mini).\n\
    AGX_ECHO_MODEL      Path to Echo model (GGUF) for Candle backend.\n\
    AGX_DELTA_MODEL     Path to Delta model (GGUF) for Candle backend.\n\
    AGQ_ADDR            AGQ TCP address (default: 127.0.0.1:6380).\n\
    AGQ_SESSION_KEY     Session key for AGQ (optional).\n\
    AGQ_TIMEOUT_SECS    Network timeout in seconds (default: 5).\n\
";

#[derive(Debug, Clone)]
pub enum Command {
    Repl,
    Plan(PlanCommand),
    Action(ActionCommand),
    Ops(OpsCommand),
}

#[derive(Debug, Clone)]
pub enum PlanCommand {
    New,
    Add { instruction: String },
    Validate,
    Preview,
    Submit { json: bool },
    List { json: bool },
    Get { plan_id: String },
}

#[derive(Debug, Clone)]
pub enum ActionCommand {
    Submit {
        plan_id: String,
        input: Option<String>,
        inputs_file: Option<String>,
        json: bool,
    },
}

#[derive(Debug, Clone)]
pub enum OpsCommand {
    Jobs { json: bool },
    Workers { json: bool },
    Queue { json: bool },
}

#[derive(Debug)]
pub struct CliConfig {
    pub command: Option<Command>,
    pub show_help: bool,
    pub show_version: bool,
    pub debug: bool,
}

impl CliConfig {
    pub fn from_env() -> Result<Self, String> {
        let args = std::env::args().skip(1);
        Self::from_args(args)
    }

    pub fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut show_help = false;
        let mut show_version = false;
        let mut debug = false;
        let mut command_tokens: Vec<String> = Vec::new();

        let mut iter = args.into_iter();

        while let Some(argument) = iter.next() {
            match argument.as_str() {
                "--help" | "-h" => {
                    show_help = true;
                }
                "--version" | "-v" => {
                    show_version = true;
                    show_help = true;
                }
                "--debug" | "-d" => {
                    debug = true;
                }
                _ => {
                    command_tokens.push(argument);
                    command_tokens.extend(iter);
                    break;
                }
            }
        }

        let command = if command_tokens.is_empty() {
            // No command means enter REPL mode (unless showing help/version)
            if !show_help && !show_version {
                Some(Command::Repl)
            } else {
                None
            }
        } else {
            Some(parse_command(&command_tokens)?)
        };

        Ok(Self {
            command,
            show_help,
            show_version,
            debug,
        })
    }
}

fn parse_command(tokens: &[String]) -> Result<Command, String> {
    if tokens.is_empty() {
        return Err("a command is required after parsing options.".to_string());
    }

    let kind = tokens[0].to_uppercase();

    match kind.as_str() {
        "PLAN" => parse_plan_command(&tokens[1..]),
        "ACTION" => parse_action_command(&tokens[1..]),
        "JOBS" | "WORKERS" | "QUEUE" => parse_ops_command(&tokens),
        _ => Err(format!(
            "unknown command: {}. Run `agx --help` for usage.",
            tokens[0]
        )),
    }
}

fn parse_plan_command(tokens: &[String]) -> Result<Command, String> {
    if tokens.is_empty() {
        return Err("PLAN requires a subcommand (new, add, validate, preview, submit).".to_string());
    }

    let sub = tokens[0].to_lowercase();

    match sub.as_str() {
        "new" => {
            if tokens.len() > 1 {
                return Err(format!(
                    "unexpected argument after `PLAN new`: {}",
                    tokens[1]
                ));
            }

            Ok(Command::Plan(PlanCommand::New))
        }
        "validate" => {
            if tokens.len() > 1 {
                return Err(format!(
                    "unexpected argument after `PLAN validate`: {}",
                    tokens[1]
                ));
            }

            Ok(Command::Plan(PlanCommand::Validate))
        }
        "preview" => {
            if tokens.len() > 1 {
                return Err(format!(
                    "unexpected argument after `PLAN preview`: {}",
                    tokens[1]
                ));
            }

            Ok(Command::Plan(PlanCommand::Preview))
        }
        "submit" => {
            let mut json = false;
            let mut i = 1;

            while i < tokens.len() {
                match tokens[i].as_str() {
                    "--json" => {
                        json = true;
                        i += 1;
                    }
                    _ => {
                        return Err(format!(
                            "unexpected argument after `PLAN submit`: {}",
                            tokens[i]
                        ));
                    }
                }
            }

            Ok(Command::Plan(PlanCommand::Submit { json }))
        }
        "add" => {
            if tokens.len() < 2 {
                return Err("PLAN add requires an instruction string.".to_string());
            }

            let instruction = tokens[1..].join(" ");
            Ok(Command::Plan(PlanCommand::Add { instruction }))
        }
        "list" => {
            let mut json = false;
            let mut i = 1;

            while i < tokens.len() {
                match tokens[i].as_str() {
                    "--json" => {
                        json = true;
                        i += 1;
                    }
                    _ => {
                        return Err(format!(
                            "unexpected argument after `PLAN list`: {}",
                            tokens[i]
                        ));
                    }
                }
            }

            Ok(Command::Plan(PlanCommand::List { json }))
        }
        "get" => {
            if tokens.len() < 2 {
                return Err("PLAN get requires a plan-id.".to_string());
            }

            if tokens.len() > 2 {
                return Err(format!(
                    "unexpected argument after `PLAN get <plan-id>`: {}",
                    tokens[2]
                ));
            }

            let plan_id = tokens[1].clone();
            Ok(Command::Plan(PlanCommand::Get { plan_id }))
        }
        _ => Err(format!(
            "unknown PLAN subcommand: {}. Expected new/add/validate/preview/submit/list/get.",
            tokens[0]
        )),
    }
}

fn parse_action_command(tokens: &[String]) -> Result<Command, String> {
    if tokens.is_empty() {
        return Err("ACTION requires a subcommand (submit).".to_string());
    }

    let sub = tokens[0].to_lowercase();

    match sub.as_str() {
        "submit" => {
            let mut plan_id = None;
            let mut input = None;
            let mut inputs_file = None;
            let mut json = false;
            let mut i = 1;

            while i < tokens.len() {
                match tokens[i].as_str() {
                    "--plan-id" => {
                        if i + 1 >= tokens.len() {
                            return Err("--plan-id requires a value".to_string());
                        }
                        plan_id = Some(tokens[i + 1].clone());
                        i += 2;
                    }
                    "--input" => {
                        if i + 1 >= tokens.len() {
                            return Err("--input requires a JSON value".to_string());
                        }
                        input = Some(tokens[i + 1].clone());
                        i += 2;
                    }
                    "--inputs-file" => {
                        if i + 1 >= tokens.len() {
                            return Err("--inputs-file requires a path".to_string());
                        }
                        inputs_file = Some(tokens[i + 1].clone());
                        i += 2;
                    }
                    "--json" => {
                        json = true;
                        i += 1;
                    }
                    other => {
                        return Err(format!("unexpected argument: {}", other));
                    }
                }
            }

            // Validate mutual exclusivity
            if input.is_some() && inputs_file.is_some() {
                return Err("cannot specify both --input and --inputs-file".to_string());
            }

            let plan_id = plan_id.ok_or_else(|| {
                "ACTION submit requires --plan-id. See `agx --help`.".to_string()
            })?;

            // Validate plan_id is not empty
            if plan_id.is_empty() {
                return Err("--plan-id cannot be empty".to_string());
            }

            Ok(Command::Action(ActionCommand::Submit {
                plan_id,
                input,
                inputs_file,
                json,
            }))
        }
        _ => Err(format!(
            "unknown ACTION subcommand: {}. Expected submit.",
            tokens[0]
        )),
    }
}

fn parse_ops_command(tokens: &[String]) -> Result<Command, String> {
    if tokens.is_empty() {
        return Err("an Ops command is required (JOBS/WORKERS/QUEUE).".to_string());
    }

    let main = tokens[0].to_uppercase();
    let mut json = false;
    let mut sub_tokens = tokens[1..].to_vec();

    if sub_tokens.contains(&"--json".to_string()) {
        json = true;
        sub_tokens.retain(|t| t != "--json");
    }

    match main.as_str() {
        "JOBS" => {
            if sub_tokens.get(0).map(|s| s.to_lowercase()) == Some("list".to_string()) {
                Ok(Command::Ops(OpsCommand::Jobs { json }))
            } else {
                Err("JOBS requires subcommand: list".to_string())
            }
        }
        "WORKERS" => {
            if sub_tokens.get(0).map(|s| s.to_lowercase()) == Some("list".to_string()) {
                Ok(Command::Ops(OpsCommand::Workers { json }))
            } else {
                Err("WORKERS requires subcommand: list".to_string())
            }
        }
        "QUEUE" => {
            if sub_tokens.get(0).map(|s| s.to_lowercase()) == Some("stats".to_string()) {
                Ok(Command::Ops(OpsCommand::Queue { json }))
            } else {
                Err("QUEUE requires subcommand: stats".to_string())
            }
        }
        _ => Err(format!("unknown Ops command: {}", tokens[0])),
    }
}

pub fn print_help() {
    println!("{HELP_TEXT}");
}

pub fn print_version() {
    println!("agx {DISPLAY_VERSION}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plan_new_command() {
        let config =
            CliConfig::from_args(vec!["PLAN".to_string(), "new".to_string()]).expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::New)) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_plan_add_command_with_spaces() {
        let config = CliConfig::from_args(vec![
            "PLAN".to_string(),
            "add".to_string(),
            "sort".to_string(),
            "and".to_string(),
            "uniq".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::Add { instruction })) => {
                assert_eq!(instruction, "sort and uniq");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn plan_add_requires_instruction() {
        let result = CliConfig::from_args(vec!["PLAN".to_string(), "add".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_plan_validate_command() {
        let config = CliConfig::from_args(vec!["PLAN".to_string(), "validate".to_string()])
            .expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::Validate)) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn plan_validate_no_extra_args() {
        let result = CliConfig::from_args(vec![
            "PLAN".to_string(),
            "validate".to_string(),
            "extra".to_string(),
        ]);
        match result {
            Err(msg) => assert!(msg.contains("unexpected argument after `PLAN validate`")),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    #[test]
    fn parse_plan_submit_without_json() {
        let config =
            CliConfig::from_args(vec!["PLAN".to_string(), "submit".to_string()]).expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::Submit { json: false })) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_plan_submit_with_json() {
        let config = CliConfig::from_args(vec![
            "PLAN".to_string(),
            "submit".to_string(),
            "--json".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::Submit { json: true })) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn plan_submit_rejects_unknown_flag() {
        let result = CliConfig::from_args(vec![
            "PLAN".to_string(),
            "submit".to_string(),
            "--unknown".to_string(),
        ]);
        match result {
            Err(msg) => assert!(msg.contains("unexpected argument after `PLAN submit`")),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    #[test]
    fn parse_plan_list_without_json() {
        let config =
            CliConfig::from_args(vec!["PLAN".to_string(), "list".to_string()]).expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::List { json: false })) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_plan_list_with_json() {
        let config = CliConfig::from_args(vec![
            "PLAN".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::List { json: true })) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_plan_get() {
        let config = CliConfig::from_args(vec![
            "PLAN".to_string(),
            "get".to_string(),
            "plan_abc123".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Plan(PlanCommand::Get { plan_id })) => {
                assert_eq!(plan_id, "plan_abc123");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn plan_get_requires_plan_id() {
        let result = CliConfig::from_args(vec!["PLAN".to_string(), "get".to_string()]);
        match result {
            Err(msg) => assert!(msg.contains("requires a plan-id")),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    #[test]
    fn plan_get_rejects_extra_args() {
        let result = CliConfig::from_args(vec![
            "PLAN".to_string(),
            "get".to_string(),
            "plan_abc123".to_string(),
            "extra".to_string(),
        ]);
        match result {
            Err(msg) => assert!(msg.contains("unexpected argument after `PLAN get")),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    #[test]
    fn parse_jobs_list_with_json_flag() {
        let config = CliConfig::from_args(vec![
            "JOBS".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Ops(OpsCommand::Jobs { json })) => assert!(json),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parse_workers_list_without_json() {
        let config =
            CliConfig::from_args(vec!["WORKERS".to_string(), "list".to_string()]).expect("valid");

        match config.command {
            Some(Command::Ops(OpsCommand::Workers { json })) => assert!(!json),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parse_queue_stats_unknown_subcommand_errors() {
        let res = CliConfig::from_args(vec![
            "QUEUE".to_string(),
            "bad".to_string(),
            "--json".to_string(),
        ]);
        assert!(res.is_err());
    }

    #[test]
    fn parse_action_submit_with_plan_id() {
        let config = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
            "plan-123".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Action(ActionCommand::Submit {
                plan_id,
                input,
                inputs_file,
                json,
            })) => {
                assert_eq!(plan_id, "plan-123");
                assert_eq!(input, None);
                assert_eq!(inputs_file, None);
                assert_eq!(json, false);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_action_submit_with_input() {
        let config = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
            "plan-123".to_string(),
            "--input".to_string(),
            "{\"key\":\"value\"}".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Action(ActionCommand::Submit {
                plan_id,
                input,
                inputs_file,
                json,
            })) => {
                assert_eq!(plan_id, "plan-123");
                assert_eq!(input, Some("{\"key\":\"value\"}".to_string()));
                assert_eq!(inputs_file, None);
                assert_eq!(json, false);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parse_action_submit_with_inputs_file() {
        let config = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
            "plan-123".to_string(),
            "--inputs-file".to_string(),
            "/path/to/inputs.json".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Action(ActionCommand::Submit {
                plan_id,
                input,
                inputs_file,
                json,
            })) => {
                assert_eq!(plan_id, "plan-123");
                assert_eq!(input, None);
                assert_eq!(inputs_file, Some("/path/to/inputs.json".to_string()));
                assert_eq!(json, false);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn action_submit_requires_plan_id() {
        let result = CliConfig::from_args(vec!["ACTION".to_string(), "submit".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--plan-id"));
    }

    #[test]
    fn action_submit_plan_id_requires_value() {
        let result = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
        ]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires a value"));
    }

    #[test]
    fn action_submit_rejects_unknown_flags() {
        let result = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
            "plan-123".to_string(),
            "--unknown".to_string(),
        ]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unexpected argument"));
    }

    #[test]
    fn action_submit_rejects_both_input_flags() {
        let result = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
            "plan-123".to_string(),
            "--input".to_string(),
            "{}".to_string(),
            "--inputs-file".to_string(),
            "file.json".to_string(),
        ]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot specify both"));
    }

    #[test]
    fn action_submit_rejects_empty_plan_id() {
        let result = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
            "".to_string(),
        ]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn parse_action_submit_with_json_flag() {
        let config = CliConfig::from_args(vec![
            "ACTION".to_string(),
            "submit".to_string(),
            "--plan-id".to_string(),
            "plan-123".to_string(),
            "--input".to_string(),
            "{\"path\":\"/tmp\"}".to_string(),
            "--json".to_string(),
        ])
        .expect("valid");

        match config.command {
            Some(Command::Action(ActionCommand::Submit {
                plan_id,
                input,
                inputs_file,
                json,
            })) => {
                assert_eq!(plan_id, "plan-123");
                assert_eq!(input, Some("{\"path\":\"/tmp\"}".to_string()));
                assert_eq!(inputs_file, None);
                assert_eq!(json, true);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
