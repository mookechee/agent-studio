use std::{cell::RefCell, collections::HashSet, rc::Rc};

use gpui::SharedString;
use gpui_component::{list::ListState, IndexPath};

use crate::schemas::workspace_schema::{Workspace, WorkspaceTask};

/// TaskListDelegate manages the task list organized by workspace
pub struct TaskListDelegate {
    /// List of workspaces
    pub workspaces: Vec<Workspace>,
    /// Tasks grouped by workspace (aligned with workspaces vec)
    pub workspace_tasks: Vec<Vec<Rc<WorkspaceTask>>>,
    /// Search query
    pub query: SharedString,
    /// Selected item index
    pub selected_index: Option<IndexPath>,
    /// Confirmed item index
    pub confirmed_index: Option<IndexPath>,
    /// Loading state
    pub loading: bool,
    /// End of data flag
    pub eof: bool,
    /// Lazy load flag
    pub lazy_load: bool,
    /// Track which workspace sections are collapsed
    pub collapsed_sections: Rc<RefCell<HashSet<usize>>>,
    /// Weak reference to list state for notifications
    pub list_state: Option<gpui::WeakEntity<ListState<TaskListDelegate>>>,
}

impl TaskListDelegate {
    pub fn new() -> Self {
        Self {
            workspaces: vec![],
            workspace_tasks: vec![],
            query: "".into(),
            selected_index: Some(IndexPath::default()),
            confirmed_index: None,
            loading: false,
            eof: false,
            lazy_load: false,
            collapsed_sections: Rc::new(RefCell::new(HashSet::new())),
            list_state: None,
        }
    }

    /// Check if a workspace section is collapsed
    pub fn is_section_collapsed(&self, section: usize) -> bool {
        self.collapsed_sections.borrow().contains(&section)
    }

    /// Prepare the task list by filtering based on query
    pub fn prepare(&mut self, query: impl Into<SharedString>) {
        self.query = query.into();
        // Note: Filtering is done during load_from_service
        // This method is kept for compatibility with ListDelegate interface
    }

    /// Load workspaces and tasks from WorkspaceService
    pub fn load_from_service(&mut self, workspaces: Vec<Workspace>, all_tasks: Vec<WorkspaceTask>) {
        let query_lower = self.query.to_lowercase();

        self.workspaces = workspaces;

        log::info!(
            "[TaskListDelegate] load_from_service: {} workspaces, {} tasks",
            self.workspaces.len(),
            all_tasks.len()
        );

        // Group tasks by workspace and apply filter
        self.workspace_tasks.clear();
        for workspace in &self.workspaces {
            let mut workspace_tasks: Vec<Rc<WorkspaceTask>> = all_tasks
                .iter()
                .filter(|task| {
                    task.workspace_id == workspace.id
                        && (query_lower.is_empty()
                            || task.name.to_lowercase().contains(&query_lower))
                })
                .map(|task| Rc::new(task.clone()))
                .collect();

            // Sort by created_at descending (newest first)
            workspace_tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            log::info!(
                "[TaskListDelegate] Workspace '{}': {} tasks",
                workspace.name,
                workspace_tasks.len()
            );

            self.workspace_tasks.push(workspace_tasks);
        }
    }

    /// Get the selected task
    pub fn selected_task(&self) -> Option<Rc<WorkspaceTask>> {
        let ix = self.selected_index?;
        self.workspace_tasks
            .get(ix.section)
            .and_then(|tasks| tasks.get(ix.row))
            .cloned()
    }

    /// Get the selected workspace
    pub fn selected_workspace(&self) -> Option<&Workspace> {
        let ix = self.selected_index?;
        self.workspaces.get(ix.section)
    }

    /// Add a task to a workspace
    pub fn add_task(&mut self, task: WorkspaceTask) {
        // Find the workspace index
        if let Some(workspace_idx) = self
            .workspaces
            .iter()
            .position(|w| w.id == task.workspace_id)
        {
            // Add task to the beginning of the workspace's task list
            if let Some(tasks) = self.workspace_tasks.get_mut(workspace_idx) {
                tasks.insert(0, Rc::new(task));
            }
        }
    }

    /// Update a task's last message by session_id
    pub fn update_task_message(&mut self, session_id: &str, message: String) -> bool {
        for workspace_tasks in &mut self.workspace_tasks {
            for task in workspace_tasks.iter_mut() {
                if task.session_id.as_ref() == Some(&session_id.to_string()) {
                    let mut updated_task = (**task).clone();
                    updated_task.update_last_message(message);
                    *task = Rc::new(updated_task);
                    return true;
                }
            }
        }
        false
    }

    /// Add a workspace
    pub fn add_workspace(&mut self, workspace: Workspace) {
        self.workspaces.push(workspace);
        self.workspace_tasks.push(vec![]);
    }
}
