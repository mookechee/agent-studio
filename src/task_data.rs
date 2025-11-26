use crate::task_schema::{AgentTask, TaskStatus};

/// Load mock agent tasks from JSON file
pub fn load_mock_tasks() -> Vec<AgentTask> {
    let json_data = include_str!("../mock_tasks.json");
    match serde_json::from_str::<Vec<AgentTask>>(json_data) {
        Ok(tasks) => tasks.into_iter().map(|task| task.prepare()).collect(),
        Err(e) => {
            eprintln!("Failed to load mock tasks: {}", e);
            Vec::new()
        }
    }
}

/// Generate a random task status for demo purposes
pub fn random_status() -> TaskStatus {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u8;
    match seed % 4 {
        0 => TaskStatus::Pending,
        1 => TaskStatus::InProgress,
        2 => TaskStatus::Completed,
        _ => TaskStatus::Failed,
    }
}
