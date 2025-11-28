use std::rc::Rc;

use gpui::{
    div, prelude::FluentBuilder, px, App, IntoElement, ParentElement, RenderOnce, Styled, Window,
};
use gpui_component::{
    h_flex, list::ListItem, spinner::Spinner, v_flex, ActiveTheme, Icon, IconName, Selectable,
    Sizable,
};

use crate::task_schema::{AgentTask, TaskStatus};

#[derive(IntoElement)]
pub struct TaskListItem {
    base: ListItem,
    agent_task: Rc<AgentTask>,
    selected: bool,
}

impl TaskListItem {
    pub fn new(id: impl Into<gpui::ElementId>, agent_task: Rc<AgentTask>, selected: bool) -> Self {
        TaskListItem {
            agent_task,
            base: ListItem::new(id).selected(selected),
            selected,
        }
    }
}

impl Selectable for TaskListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for TaskListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = if self.selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let muted_color = cx.theme().muted_foreground;
        let add_color = cx.theme().green;
        let delete_color = cx.theme().red;

        // Show metadata only when not selected
        let _show_metadata = !self.selected;

        // Check if task is in progress to use Spinner
        let is_in_progress = matches!(self.agent_task.status, TaskStatus::InProgress);

        self.base
            .px_3()
            .py_2()
            .overflow_x_hidden()
            .rounded(cx.theme().radius)
            .child(
                h_flex()
                    .items_start() // Top align instead of center
                    .gap_3()
                    .mt(px(2.))
                    .child(div().mt(px(2.)).map(|this| {
                        if is_in_progress {
                            // Use Spinner for InProgress status
                            this.child(Spinner::new().with_size(px(14.)).color(cx.theme().accent))
                        } else {
                            // Use Icon for other statuses
                            let (icon_name, icon_color) = match self.agent_task.status {
                                TaskStatus::Pending => (IconName::File, muted_color),
                                TaskStatus::Completed => (IconName::CircleCheck, cx.theme().green),
                                TaskStatus::Failed => (IconName::CircleX, cx.theme().red),
                                _ => (IconName::File, muted_color),
                            };
                            this.child(Icon::new(icon_name).text_color(icon_color).size(px(14.)))
                        }
                    }))
                    .child(
                        // Vertical layout for title and subtitle
                        v_flex()
                            .gap_0p5()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                // Title - reduced font size
                                div()
                                    .text_size(px(13.))
                                    .text_color(text_color)
                                    .whitespace_nowrap()
                                    .child(self.agent_task.name.clone()),
                            )
                            .when_some(self.agent_task.subtitle.clone(), |this, subtitle| {
                                // Show message preview if available
                                this.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(muted_color)
                                        .whitespace_nowrap()
                                        .overflow_x_hidden()
                                        .child(subtitle),
                                )
                            })
                            .when(self.agent_task.subtitle.is_none(), |this| {
                                // Fallback to metadata display when no subtitle
                                this.child(
                                    h_flex()
                                        .gap_1()
                                        .text_size(px(11.))
                                        .text_color(muted_color)
                                        .child("2 Files ")
                                        .child(
                                            div()
                                                .text_color(add_color)
                                                .child(self.agent_task.add_new_code_lines_str.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_color(delete_color)
                                                .child(self.agent_task.delete_code_lines_str.clone()),
                                        )
                                        .child(" · ")
                                        .child(self.agent_task.task_type.clone()),
                                )
                            }),
                    ),
            )
    }
}
