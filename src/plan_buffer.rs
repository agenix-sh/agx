use std::fs;
use std::path::{Path, PathBuf};

use crate::plan::WorkflowPlan;

pub struct PlanStorage {
    path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanMetadata {
    pub job_id: String,
    pub submitted_at: String,
}

impl PlanStorage {
    pub fn from_env() -> Self {
        if let Ok(path) = std::env::var("AGX_PLAN_PATH") {
            return Self::new(PathBuf::from(path));
        }

        let mut path = std::env::temp_dir();
        path.push("agx-plan.json");

        Self::new(path)
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reset(&self) -> Result<WorkflowPlan, String> {
        let plan = WorkflowPlan::default();
        self.save(&plan)?;
        Ok(plan)
    }

    pub fn load(&self) -> Result<WorkflowPlan, String> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => {
                if contents.trim().is_empty() {
                    return Ok(WorkflowPlan::default());
                }

                serde_json::from_str(&contents).map_err(|error| {
                    format!(
                        "failed to parse plan buffer {}: {error}",
                        self.display_path()
                    )
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(WorkflowPlan::default())
            }
            Err(error) => Err(format!(
                "failed to read plan buffer {}: {error}",
                self.display_path()
            )),
        }
    }

    pub fn save(&self, plan: &WorkflowPlan) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "failed to create plan directory {}: {error}",
                        parent.display()
                    )
                })?;
            }
        }

        let json = serde_json::to_string_pretty(plan)
            .map_err(|error| format!("failed to serialize plan buffer: {error}"))?;

        fs::write(&self.path, json).map_err(|error| {
            format!(
                "failed to write plan buffer {}: {error}",
                self.display_path()
            )
        })
    }

    pub fn save_submission_metadata(&self, metadata: &PlanMetadata) -> Result<(), String> {
        let path = self.metadata_path();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "failed to create plan metadata directory {}: {error}",
                        parent.display()
                    )
                })?;
            }
        }

        let json = serde_json::to_string_pretty(metadata)
            .map_err(|error| format!("failed to serialize submission metadata: {error}"))?;

        fs::write(&path, json).map_err(|error| {
            format!(
                "failed to write plan metadata {}: {error}",
                path.as_os_str().to_string_lossy()
            )
        })
    }

    fn metadata_path(&self) -> PathBuf {
        let mut path = self.path.clone();
        let new_extension = match path.extension() {
            Some(ext) => {
                let mut os = ext.to_os_string();
                os.push(".meta");
                os
            }
            None => std::ffi::OsString::from("meta"),
        };
        path.set_extension(new_extension);
        path
    }

    fn display_path(&self) -> String {
        self.path.as_os_str().to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::plan::{PlanStep, WorkflowPlan};

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let pid = std::process::id();
        path.push(format!("agx-plan-test-{pid}-{name}.json"));
        path
    }

    #[test]
    fn load_returns_empty_when_missing() {
        let path = temp_path("missing");

        if path.exists() {
            fs::remove_file(&path).unwrap();
        }

        let storage = PlanStorage::new(path.clone());
        let plan = storage.load().expect("missing file should load as empty");

        assert_eq!(plan.tasks.len(), 0);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let path = temp_path("roundtrip");

        if path.exists() {
            fs::remove_file(&path).unwrap();
        }

        let storage = PlanStorage::new(path.clone());
        let plan = WorkflowPlan {
            plan_id: None,
            plan_description: None,
            tasks: vec![PlanStep {
                task_number: 1,
                command: "sort".to_string(),
                args: vec!["-r".to_string()],
                timeout_secs: 300,
                input_from_task: None,
            }],
        };

        storage.save(&plan).expect("save should succeed");

        let loaded = storage.load().expect("load should succeed");
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].command, "sort");
        assert_eq!(loaded.tasks[0].args, vec!["-r"]);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn save_submission_metadata_writes_file() {
        let path = temp_path("meta");
        let storage = PlanStorage::new(path.clone());
        let meta = PlanMetadata {
            job_id: "job-123".to_string(),
            submitted_at: "2025-11-15T00:00:00Z".to_string(),
        };

        storage
            .save_submission_metadata(&meta)
            .expect("metadata should save");

        let meta_path = storage.metadata_path();
        assert!(meta_path.exists());

        let contents = std::fs::read_to_string(meta_path).unwrap();
        assert!(contents.contains("job-123"));
        fs::remove_file(path.with_extension("json.meta")).unwrap();
    }

    #[test]
    fn metadata_errors_surface() {
        let path = PathBuf::from("/root/forbidden/agx-plan.json");
        let storage = PlanStorage::new(path);
        let meta = PlanMetadata {
            job_id: "job-err".to_string(),
            submitted_at: "2025-11-15T00:00:00Z".to_string(),
        };

        let result = storage.save_submission_metadata(&meta);
        assert!(result.is_err());
    }
}
