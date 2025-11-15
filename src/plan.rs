use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlan {
    pub plan: Vec<PlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SimpleWorkflowPlan {
    plan: Vec<String>,
}

impl Default for WorkflowPlan {
    fn default() -> Self {
        Self { plan: Vec::new() }
    }
}

impl WorkflowPlan {
    pub fn from_str(value: &str) -> Result<Self, serde_json::Error> {
        let cleaned = strip_markdown_fence(value);
        parse_any_form(&cleaned)
    }

    pub fn normalize_for_execution(mut self) -> Self {
        if self.plan.len() == 1 && self.plan[0].cmd == "uniq" {
            self.plan = vec![
                PlanStep {
                    cmd: "sort".to_string(),
                    args: Vec::new(),
                },
                PlanStep {
                    cmd: "uniq".to_string(),
                    args: Vec::new(),
                },
            ];
        }

        self
    }
}

fn strip_markdown_fence(value: &str) -> String {
    let trimmed = value.trim();

    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines();
    lines.next();

    let mut body = String::new();

    for line in lines {
        if line.trim_start().starts_with("```") {
            break;
        }

        if !body.is_empty() {
            body.push('\n');
        }

        body.push_str(line);
    }

    body
}

fn parse_any_form(text: &str) -> Result<WorkflowPlan, serde_json::Error> {
    if let Some(plan) = try_all_known_forms(text) {
        return Ok(plan);
    }

    if let Some(extracted) = extract_first_json_value(text) {
        if let Some(plan) = try_all_known_forms(extracted) {
            return Ok(plan);
        }
    }

    let repaired = repair_unescaped_quotes(text);
    if repaired != text {
        if let Some(plan) = try_all_known_forms(&repaired) {
            return Ok(plan);
        }

        if let Some(extracted) = extract_first_json_value(&repaired) {
            if let Some(plan) = try_all_known_forms(extracted) {
                return Ok(plan);
            }
        }
    }

    serde_json::from_str::<WorkflowPlan>(text)
}

fn try_all_known_forms(text: &str) -> Option<WorkflowPlan> {
    if let Ok(plan) = serde_json::from_str::<WorkflowPlan>(text) {
        return Some(plan);
    }

    if let Ok(simple) = serde_json::from_str::<SimpleWorkflowPlan>(text) {
        return Some(WorkflowPlan {
            plan: simple
                .plan
                .into_iter()
                .map(|cmd| PlanStep {
                    cmd,
                    args: Vec::new(),
                })
                .collect(),
        });
    }

    if let Ok(steps) = serde_json::from_str::<Vec<PlanStep>>(text) {
        return Some(WorkflowPlan { plan: steps });
    }

    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(text) {
        return Some(WorkflowPlan {
            plan: cmds
                .into_iter()
                .map(|cmd| PlanStep {
                    cmd,
                    args: Vec::new(),
                })
                .collect(),
        });
    }

    None
}

fn extract_first_json_value(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let mut start = None;
    let mut depth = 0;

    for (index, character) in trimmed.char_indices() {
        if start.is_none() {
            if character == '{' || character == '[' {
                start = Some(index);
                depth = 1;
            }

            continue;
        }

        if character == '{' || character == '[' {
            depth += 1;
        } else if character == '}' || character == ']' {
            depth -= 1;

            if depth == 0 {
                let end = index + character.len_utf8();
                let begin = start.unwrap();

                return Some(&trimmed[begin..end]);
            }
        }
    }

    None
}

fn repair_unescaped_quotes(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if ch == '"' {
            if !in_string {
                in_string = true;
                output.push(ch);
                continue;
            }

            if escaped {
                output.push(ch);
                escaped = false;
                continue;
            }

            let mut lookahead = chars.clone();
            let mut next_non_ws = None;

            while let Some(next) = lookahead.next() {
                if !next.is_whitespace() {
                    next_non_ws = Some(next);
                    break;
                }
            }

            let should_escape = match next_non_ws {
                Some(',') | Some(']') | Some('}') | Some(':') | None => false,
                _ => true,
            };

            if should_escape {
                output.push('\\');
                output.push('"');

                if matches!(chars.peek(), Some('/')) {
                    let mut lookahead_after_slash = chars.clone();
                    lookahead_after_slash.next();

                    while let Some(next_char) = lookahead_after_slash.next() {
                        if next_char.is_whitespace() {
                            continue;
                        }

                        if matches!(next_char, ',' | ']' | '}') {
                            chars.next();
                        }

                        break;
                    }
                }

                continue;
            } else {
                in_string = false;
                output.push('"');
                continue;
            }
        }

        if ch == '\\' {
            if in_string && !escaped {
                escaped = true;
            } else {
                escaped = false;
            }
        } else {
            escaped = false;
        }

        output.push(ch);

        if !in_string {
            escaped = false;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_unescaped_quotes_in_args() {
        let broken = r#"{
            "plan": [
                {"cmd": "sort"},
                {"cmd": "awk", "args": ["-F"/"/,NR==1;print length($NF)"]}
            ]
        }"#;

        let plan = WorkflowPlan::from_str(broken).expect("plan should be repaired");
        assert_eq!(plan.plan.len(), 2);
        assert_eq!(plan.plan[1].cmd, "awk");
        assert_eq!(
            plan.plan[1].args,
            vec!["-F\"/\",NR==1;print length($NF)".to_string()]
        );
    }

    #[test]
    fn leaves_valid_json_unchanged() {
        let valid = r#"{"plan":[{"cmd":"cat","args":["file.txt"]}]}"#;
        let repaired = repair_unescaped_quotes(valid);

        assert_eq!(valid, repaired);
    }
}
