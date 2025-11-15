const DISPLAY_VERSION: &str = "0.1";

const HELP_TEXT: &str = "\
AGX - Agentic planner CLI (Phase 1)\n\
\n\
Usage:\n\
    agx [OPTIONS] PLAN <subcommand>\n\
\n\
PLAN subcommands:\n\
    PLAN new                 Reset the persisted plan buffer.\n\
    PLAN add \"<instruction>\"  Append planner-generated steps. Reads STDIN when piped.\n\
    PLAN preview             Pretty-print the current JSON plan buffer.\n\
    PLAN submit              Validate the plan and prepare it for AGQ submission.\n\
\n\
Options:\n\
    -h, --help        Print this help text.\n\
    -v, --version     Show the version and this help output.\n\
    -d, --debug       Enable verbose logging to stderr.\n\
\n\
Environment variables:\n\
    AGX_PLAN_PATH     Override the plan buffer location (default: $TMPDIR/agx-plan.json).\n\
    AGX_BACKEND       Planner backend (ollama or embedded).\n\
    AGX_OLLAMA_MODEL  Ollama model to run when using the Ollama backend (default: phi3:mini).\n\
    AGX_MODEL_PATH    Filesystem path to a local model when using the embedded backend.\n\
    AGX_MODEL_ARCH    Architecture for embedded models (for example: llama).\n\
";

#[derive(Debug, Clone)]
pub enum Command {
    Plan(PlanCommand),
}

#[derive(Debug, Clone)]
pub enum PlanCommand {
    New,
    Add { instruction: String },
    Preview,
    Submit,
}

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
            None
        } else {
            Some(parse_command(&command_tokens)?)
        };

        if command.is_none() && !show_help && !show_version {
            return Err(
                "a command is required. Run `agx --help` or `agx -v` for usage information."
                    .to_string(),
            );
        }

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
        _ => Err(format!(
            "unknown command: {}. Run `agx --help` for usage.",
            tokens[0]
        )),
    }
}

fn parse_plan_command(tokens: &[String]) -> Result<Command, String> {
    if tokens.is_empty() {
        return Err("PLAN requires a subcommand (new, add, preview, submit).".to_string());
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
            if tokens.len() > 1 {
                return Err(format!(
                    "unexpected argument after `PLAN submit`: {}",
                    tokens[1]
                ));
            }

            Ok(Command::Plan(PlanCommand::Submit))
        }
        "add" => {
            if tokens.len() < 2 {
                return Err("PLAN add requires an instruction string.".to_string());
            }

            let instruction = tokens[1..].join(" ");
            Ok(Command::Plan(PlanCommand::Add { instruction }))
        }
        _ => Err(format!(
            "unknown PLAN subcommand: {}. Expected new/add/preview/submit.",
            tokens[0]
        )),
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
}
