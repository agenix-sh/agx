use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WorkflowPlan {
    pub plan: Vec<PlanStep>,
}

#[derive(Debug, Deserialize)]
pub struct PlanStep {
    pub cmd: String,
}

#[derive(Debug, Deserialize)]
struct SimpleWorkflowPlan {
    plan: Vec<String>,
}

impl WorkflowPlan {
    pub fn from_str(value: &str) -> Result<Self, serde_json::Error> {
        let cleaned = strip_markdown_fence(value);

        match serde_json::from_str::<WorkflowPlan>(&cleaned) {
            Ok(plan) => Ok(plan),
            Err(_) => {
                let simple: SimpleWorkflowPlan = serde_json::from_str(&cleaned)?;

                Ok(WorkflowPlan {
                    plan: simple
                        .plan
                        .into_iter()
                        .map(|cmd| PlanStep { cmd })
                        .collect(),
                })
            }
        }
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

