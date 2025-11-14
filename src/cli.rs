pub struct CliConfig {
    pub instruction: String,
}

impl CliConfig {
    pub fn from_env() -> Result<Self, String> {
        let mut arguments = std::env::args().skip(1);

        let instruction = match arguments.next() {
            Some(value) => value,
            None => {
                return Err("Usage: agx <instruction>".to_string());
            }
        };

        Ok(Self { instruction })
    }
}

