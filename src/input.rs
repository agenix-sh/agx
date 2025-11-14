use std::io::{self, Read};

pub struct InputSummary {
    pub bytes: usize,
    pub lines: usize,
    pub is_empty: bool,
    pub is_probably_binary: bool,
    pub content: Vec<u8>,
}

pub struct InputCollector;

impl InputCollector {
    pub fn collect() -> Result<InputSummary, io::Error> {
        let mut content = Vec::new();
        io::stdin().read_to_end(&mut content)?;

        let bytes = content.len();
        let lines = if bytes == 0 {
            0
        } else {
            content.iter().filter(|&&byte| byte == b'\n').count() + 1
        };
        let is_empty = bytes == 0;
        let is_probably_binary = content.iter().any(|&byte| byte == b'\0');

        Ok(InputSummary {
            bytes,
            lines,
            is_empty,
            is_probably_binary,
            content,
        })
    }
}

