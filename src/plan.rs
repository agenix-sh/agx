use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WorkflowPlan {
    pub plan: Vec<PlanStep>,
}

#[derive(Debug, Deserialize)]
pub struct PlanStep {
    pub cmd: String,
}

impl WorkflowPlan {
    pub fn from_str(value: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(value)
    }
}

