use std::io::{self, IsTerminal, Read};

pub struct InputSummary {
    pub bytes: usize,
    pub lines: usize,
    pub is_empty: bool,
    pub is_probably_binary: bool,
    pub content: Vec<u8>,
}

pub struct InputCollector;

impl InputSummary {
    pub fn empty() -> Self {
        Self {
            bytes: 0,
            lines: 0,
            is_empty: true,
            is_probably_binary: false,
            content: Vec::new(),
        }
    }
}

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

    pub fn stdin_is_terminal() -> bool {
        io::stdin().is_terminal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_binary_content() {
        let summary = InputSummary {
            bytes: 1,
            lines: 1,
            is_empty: false,
            is_probably_binary: [0u8].iter().any(|&b| b == b'\0'),
            content: vec![0],
        };

        assert!(summary.is_probably_binary);
    }

    #[test]
    fn marks_empty_input() {
        let summary = InputSummary {
            bytes: 0,
            lines: 0,
            is_empty: true,
            is_probably_binary: false,
            content: Vec::new(),
        };

        assert!(summary.is_empty);
        assert!(!summary.is_probably_binary);
    }
}
