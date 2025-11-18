use serde::{Deserialize, Serialize};

use crate::plan::WorkflowPlan;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnvelope {
    pub job_id: String,
    pub plan_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_description: Option<String>,
    pub tasks: Vec<JobTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTask {
    pub task_number: u32,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_from_task: Option<u32>,
}

fn default_timeout() -> u32 {
    300
}

#[derive(Debug)]
pub enum EnvelopeValidationError {
    EmptyTasks,
    TooManyTasks(usize),
    NonMonotonicTasks,
    BadInputReference(u32),
    FirstTaskNotOne(u32),
}

impl std::fmt::Display for EnvelopeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeValidationError::EmptyTasks => write!(f, "plan contains no tasks"),
            EnvelopeValidationError::TooManyTasks(count) => {
                write!(f, "plan has too many tasks ({count})")
            }
            EnvelopeValidationError::NonMonotonicTasks => {
                write!(f, "task numbers must be contiguous starting at 1")
            }
            EnvelopeValidationError::BadInputReference(task) => {
                write!(f, "input_from_task references invalid task {task}")
            }
            EnvelopeValidationError::FirstTaskNotOne(n) => {
                write!(f, "first task number must be 1 (found {n})")
            }
        }
    }
}

impl JobEnvelope {
    pub fn from_plan(
        plan: WorkflowPlan,
        job_id: String,
        plan_id_override: String,
        plan_description_override: Option<String>,
    ) -> Self {
        // Use plan's IDs if provided, otherwise use overrides
        let plan_id = plan.plan_id.unwrap_or(plan_id_override);
        let plan_description = plan.plan_description.or(plan_description_override);

        // Convert tasks and ensure proper numbering (defensive: normalize_for_execution should have done this)
        let tasks: Vec<JobTask> = plan
            .tasks
            .into_iter()
            .enumerate()
            .map(|(index, task)| JobTask {
                task_number: (index + 1) as u32, // Ensure contiguous 1-based numbering
                command: task.command,
                args: task.args,
                timeout_secs: task.timeout_secs,
                input_from_task: task.input_from_task,
            })
            .collect();

        Self {
            job_id,
            plan_id,
            plan_description,
            tasks,
        }
    }

    pub fn validate(&self, max_tasks: usize) -> Result<(), EnvelopeValidationError> {
        if self.tasks.is_empty() {
            return Err(EnvelopeValidationError::EmptyTasks);
        }

        if self.tasks[0].task_number != 1 {
            return Err(EnvelopeValidationError::FirstTaskNotOne(
                self.tasks[0].task_number,
            ));
        }

        if self.tasks.len() > max_tasks {
            return Err(EnvelopeValidationError::TooManyTasks(self.tasks.len()));
        }

        for window in self.tasks.windows(2) {
            if window[0].task_number + 1 != window[1].task_number {
                return Err(EnvelopeValidationError::NonMonotonicTasks);
            }
        }

        let mut seen = std::collections::HashSet::new();
        for task in &self.tasks {
            seen.insert(task.task_number);
            if let Some(ref_id) = task.input_from_task {
                if ref_id >= task.task_number || !seen.contains(&ref_id) {
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
    fn builds_envelope_with_task_numbers() {
        let plan = WorkflowPlan {
            plan_id: None,
            plan_description: None,
            tasks: vec![
                PlanStep {
                    task_number: 1,
                    command: "sort".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: None,
                },
                PlanStep {
                    task_number: 2,
                    command: "uniq".into(),
                    args: vec![],
                    timeout_secs: 30,
                    input_from_task: Some(1),
                },
            ],
        };

        let env =
            JobEnvelope::from_plan(plan, "job-1".into(), "plan-1".into(), Some("desc".into()));
        assert_eq!(env.tasks.len(), 2);
        assert_eq!(env.tasks[0].task_number, 1);
        assert_eq!(env.tasks[1].task_number, 2);
        assert_eq!(env.tasks[1].input_from_task, Some(1));
        assert_eq!(env.tasks[1].timeout_secs, 30);
    }

    #[test]
    fn validates_monotonic_tasks() {
        let env = JobEnvelope {
            job_id: "job".into(),
            plan_id: "plan".into(),
            plan_description: None,
            tasks: vec![
                JobTask {
                    task_number: 1,
                    command: "c".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: None,
                },
                JobTask {
                    task_number: 3,
                    command: "c".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: None,
                },
            ],
        };

        let err = env.validate(10).unwrap_err();
        matches!(err, EnvelopeValidationError::NonMonotonicTasks);
    }

    #[test]
    fn rejects_invalid_input_refs() {
        let env = JobEnvelope {
            job_id: "job".into(),
            plan_id: "plan".into(),
            plan_description: None,
            tasks: vec![
                JobTask {
                    task_number: 1,
                    command: "c".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: None,
                },
                JobTask {
                    task_number: 2,
                    command: "c".into(),
                    args: vec![],
                    timeout_secs: 300,
                    input_from_task: Some(5),
                },
            ],
        };

        let err = env.validate(10).unwrap_err();
        matches!(err, EnvelopeValidationError::BadInputReference(_));
    }
}
