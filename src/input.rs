use std::io::{self, Read};

pub struct InputSummary {
    pub bytes: usize,
    pub lines: usize,
    pub is_empty: bool,
    pub is_probably_binary: bool,
    pub content: String,
}

pub struct InputCollector;

impl InputCollector {
    pub fn collect() -> Result<InputSummary, io::Error> {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

        let bytes = buffer.as_bytes().len();
        let lines = if buffer.is_empty() {
            0
        } else {
            buffer.lines().count()
        };
        let is_empty = bytes == 0;
        let is_probably_binary = buffer.chars().any(|character| character == '\u{0}');

        Ok(InputSummary {
            bytes,
            lines,
            is_empty,
            is_probably_binary,
            content: buffer,
        })
    }
}

