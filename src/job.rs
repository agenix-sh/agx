use serde::{Deserialize, Serialize};

use crate::plan::WorkflowPlan;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnvelope {
    pub job_id: String,
    pub plan_id: String,
    #[serde(default)]
    pub plan_description: Option<String>,
    pub steps: Vec<JobStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStep {
    pub step_number: u32,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub input_from_step: Option<u32>,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
}

#[derive(Debug)]
pub enum EnvelopeValidationError {
    EmptySteps,
    TooManySteps(usize),
    NonMonotonicSteps,
    BadInputReference(u32),
    FirstStepNotOne(u32),
}

impl std::fmt::Display for EnvelopeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeValidationError::EmptySteps => write!(f, "plan contains no steps"),
            EnvelopeValidationError::TooManySteps(count) => {
                write!(f, "plan has too many steps ({count})")
            }
            EnvelopeValidationError::NonMonotonicSteps => {
                write!(f, "step numbers must be contiguous starting at 1")
            }
            EnvelopeValidationError::BadInputReference(step) => {
                write!(f, "input_from_step references invalid step {step}")
            }
            EnvelopeValidationError::FirstStepNotOne(n) => {
                write!(f, "first step number must be 1 (found {n})")
            }
        }
    }
}

impl JobEnvelope {
    pub fn from_plan(
        plan: WorkflowPlan,
        job_id: String,
        plan_id: String,
        plan_description: Option<String>,
    ) -> Self {
        let steps = plan
            .plan
            .into_iter()
            .enumerate()
            .map(|(idx, step)| JobStep {
                step_number: (idx + 1) as u32,
                command: step.cmd,
                args: step.args,
                input_from_step: step.input_from_step,
                timeout_secs: step.timeout_secs,
            })
            .collect();

        Self {
            job_id,
            plan_id,
            plan_description,
            steps,
        }
    }

    pub fn validate(&self, max_steps: usize) -> Result<(), EnvelopeValidationError> {
        if self.steps.is_empty() {
            return Err(EnvelopeValidationError::EmptySteps);
        }

        if self.steps[0].step_number != 1 {
            return Err(EnvelopeValidationError::FirstStepNotOne(
                self.steps[0].step_number,
            ));
        }

        if self.steps.len() > max_steps {
            return Err(EnvelopeValidationError::TooManySteps(self.steps.len()));
        }

        for window in self.steps.windows(2) {
            if window[0].step_number + 1 != window[1].step_number {
                return Err(EnvelopeValidationError::NonMonotonicSteps);
            }
        }

        let mut seen = std::collections::HashSet::new();
        for step in &self.steps {
            seen.insert(step.step_number);
            if let Some(ref_id) = step.input_from_step {
                if ref_id >= step.step_number || !seen.contains(&ref_id) {
                    return Err(EnvelopeValidationError::BadInputReference(ref_id));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlanStep;

    #[test]
    fn builds_envelope_with_step_numbers() {
        let plan = WorkflowPlan {
            plan: vec![
                PlanStep {
                    cmd: "sort".into(),
                    args: vec![],
                    input_from_step: None,
                    timeout_secs: None,
                },
                PlanStep {
                    cmd: "uniq".into(),
                    args: vec![],
                    input_from_step: Some(1),
                    timeout_secs: Some(30),
                },
            ],
        };

        let env =
            JobEnvelope::from_plan(plan, "job-1".into(), "plan-1".into(), Some("desc".into()));
        assert_eq!(env.steps.len(), 2);
        assert_eq!(env.steps[0].step_number, 1);
        assert_eq!(env.steps[1].step_number, 2);
        assert_eq!(env.steps[1].input_from_step, Some(1));
        assert_eq!(env.steps[1].timeout_secs, Some(30));
    }

    #[test]
    fn validates_monotonic_steps() {
        let env = JobEnvelope {
            job_id: "job".into(),
            plan_id: "plan".into(),
            plan_description: None,
            steps: vec![
                JobStep {
                    step_number: 1,
                    command: "c".into(),
                    args: vec![],
                    input_from_step: None,
                    timeout_secs: None,
                },
                JobStep {
                    step_number: 3,
                    command: "c".into(),
                    args: vec![],
                    input_from_step: None,
                    timeout_secs: None,
                },
            ],
        };

        let err = env.validate(10).unwrap_err();
        matches!(err, EnvelopeValidationError::NonMonotonicSteps);
    }

    #[test]
    fn rejects_invalid_input_refs() {
        let env = JobEnvelope {
            job_id: "job".into(),
            plan_id: "plan".into(),
            plan_description: None,
            steps: vec![
                JobStep {
                    step_number: 1,
                    command: "c".into(),
                    args: vec![],
                    input_from_step: None,
                    timeout_secs: None,
                },
                JobStep {
                    step_number: 2,
                    command: "c".into(),
                    args: vec![],
                    input_from_step: Some(5),
                    timeout_secs: None,
                },
            ],
        };

        let err = env.validate(10).unwrap_err();
        matches!(err, EnvelopeValidationError::BadInputReference(_));
    }
}
