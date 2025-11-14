const DISPLAY_VERSION: &str = "0.1";

const HELP_TEXT: &str = "\
AGX - AI command execution helper\n\
\n\
Usage:\n\
    agx [OPTIONS] <instruction>\n\
\n\
Options:\n\
    -h, --help        Print this help text.\n\
    -v, --version     Show the version and this help output.\n\
    -d, --debug       Enable verbose logging to stderr.\n\
\n\
Environment variables:\n\
    AGX_BACKEND       Selects the planner backend (ollama or embedded).\n\
    AGX_OLLAMA_MODEL  Ollama model to run when using the Ollama backend (default: phi3:mini).\n\
    AGX_MODEL_PATH    Filesystem path to a local model when using the embedded backend.\n\
    AGX_MODEL_ARCH    Architecture for embedded models (for example: llama).\n\
";

pub struct CliConfig {
    pub instruction: Option<String>,
    pub show_help: bool,
    pub show_version: bool,
    pub debug: bool,
}

impl CliConfig {
    pub fn from_env() -> Result<Self, String> {
        let mut instruction: Option<String> = None;
        let mut show_help = false;
        let mut show_version = false;
        let mut debug = false;

        let mut arguments = std::env::args().skip(1);

        while let Some(argument) = arguments.next() {
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
                "--" => {
                    let rest: Vec<String> = arguments.collect();

                    if !rest.is_empty() {
                        instruction = Some(rest.join(" "));
                    }

                    break;
                }
                _ if argument.starts_with('-') => {
                    return Err(format!(
                        "unknown option: {argument}\n\nRun `agx --help` or `agx -v` for more information."
                    ));
                }
                _ => {
                    if instruction.is_none() {
                        instruction = Some(argument);
                    } else {
                        return Err(
                            "multiple instructions provided. Quote the instruction if it contains spaces."
                                .to_string(),
                        );
                    }
                }
            }
        }

        Ok(Self {
            instruction,
            show_help,
            show_version,
            debug,
        })
    }
}

pub fn print_help() {
    println!("{HELP_TEXT}");
}

pub fn print_version() {
    println!("agx {DISPLAY_VERSION}");
}
