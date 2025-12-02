use std::rc::Rc;

use gpui::{div, hsla, InteractiveElement, IntoElement, ParentElement, Styled};
use gpui_component::{h_flex, IndexPath, Selectable, StyledExt};

use crate::schemas::workspace_schema::WorkspaceTask;

/// Workspace task list item component
pub struct WorkspaceTaskItem {
    index: IndexPath,
    task: Option<Rc<WorkspaceTask>>,
    selected: bool,
    is_placeholder: bool,
}

impl WorkspaceTaskItem {
    pub fn new(index: IndexPath, task: Rc<WorkspaceTask>, selected: bool) -> Self {
        Self {
            index,
            task: Some(task),
            selected,
            is_placeholder: false,
        }
    }

    /// Create a placeholder item for empty workspaces
    pub fn placeholder(index: IndexPath, selected: bool) -> Self {
        Self {
            index,
            task: None,
            selected,
            is_placeholder: true,
        }
    }
}

impl Selectable for WorkspaceTaskItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl IntoElement for WorkspaceTaskItem {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        // Render placeholder for empty workspaces
        if self.is_placeholder {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .px_3()
                .py_6()
                .text_sm()
                .text_color(hsla(0.0, 0.0, 0.5, 1.0))
                .child("No tasks yet. Click 'New Task' to create one.");
        }

        let Some(task) = self.task else {
            // Fallback in case task is None but not a placeholder
            return div().child("Invalid task");
        };

        let mut element = div().flex().flex_col().gap_1().px_2().py_2().rounded_md();

        // Apply selected or hover styles
        element = if self.selected {
            element
                .bg(hsla(217.0 / 360.0, 0.91, 0.60, 0.1))
                .border_1()
                .border_color(hsla(217.0 / 360.0, 0.91, 0.60, 0.3))
        } else {
            element.hover(|style| style.bg(hsla(0.0, 0.0, 0.95, 0.5)))
        };

        // Task name
        element = element.child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(hsla(0.0, 0.0, 0.1, 1.0))
                .child(task.name.clone()),
        );

        // Last message preview (if available)
        if let Some(msg) = &task.last_message {
            element = element.child(
                div()
                    .text_xs()
                    .text_color(hsla(0.0, 0.0, 0.4, 1.0))
                    .child(msg.clone()),
            );
        }

        // Task metadata (agent, mode, status)
        element.child(
            h_flex()
                .gap_2()
                .text_xs()
                .text_color(hsla(0.0, 0.0, 0.6, 1.0))
                .child(task.agent_name.to_string())
                .child("·")
                .child(task.mode.to_string())
                .child("·")
                .child(format!("{:?}", task.status)),
        )
    }
}
