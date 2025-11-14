pub struct Tool {
    pub id: &'static str,
    pub command: &'static str,
    pub description: &'static str,
    pub patterns: &'static [&'static str],
    pub ok_exit_codes: &'static [i32],
}

pub struct ToolRegistry;

impl ToolRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn tools(&self) -> &'static [Tool] {
        TOOLS
    }

    pub fn find_by_id(&self, id: &str) -> Option<&'static Tool> {
        self.tools().iter().find(|tool| tool.id == id)
    }
}

static TOOLS: &[Tool] = &[
    Tool {
        id: "sort",
        command: "sort",
        description: "Sort lines of text.",
        patterns: &["sort", "order", "alphabetize", "sort lines"],
        ok_exit_codes: &[0],
    },
    Tool {
        id: "uniq",
        command: "uniq",
        description: "Remove duplicate lines.",
        patterns: &["dedupe", "unique", "remove duplicates"],
        ok_exit_codes: &[0],
    },
    Tool {
        id: "grep",
        command: "grep",
        description: "Filter lines that match a pattern.",
        patterns: &["search", "filter", "match", "grep"],
        ok_exit_codes: &[0, 1],
    },
    Tool {
        id: "cut",
        command: "cut",
        description: "Extract fields or columns from lines.",
        patterns: &["columns", "fields", "delimiter", "extract columns"],
        ok_exit_codes: &[0],
    },
    Tool {
        id: "tr",
        command: "tr",
        description: "Translate or delete characters in text.",
        patterns: &["translate", "replace characters", "lowercase", "uppercase"],
        ok_exit_codes: &[0],
    },
];

