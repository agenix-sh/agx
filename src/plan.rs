use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WorkflowPlan {
    pub plan: Vec<PlanStep>,
}

#[derive(Debug, Deserialize)]
pub struct PlanStep {
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SimpleWorkflowPlan {
    plan: Vec<String>,
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
    if let Ok(plan) = serde_json::from_str::<WorkflowPlan>(text) {
        return Ok(plan);
    }

    if let Ok(simple) = serde_json::from_str::<SimpleWorkflowPlan>(text) {
        return Ok(WorkflowPlan {
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
        return Ok(WorkflowPlan { plan: steps });
    }

    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(text) {
        return Ok(WorkflowPlan {
            plan: cmds
                .into_iter()
                .map(|cmd| PlanStep {
                    cmd,
                    args: Vec::new(),
                })
                .collect(),
        });
    }

    if let Some(extracted) = extract_first_json_value(text) {
        if extracted != text {
            return parse_any_form(extracted);
        }
    }

    serde_json::from_str::<WorkflowPlan>(text)
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
