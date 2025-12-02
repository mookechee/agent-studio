use std::time::Duration;

use gpui::{
    div, px, App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    Task, Timer, Window,
};
use gpui_component::{
    h_flex,
    list::{ListDelegate, ListState},
    v_flex, ActiveTheme, Icon, IconName, IndexPath,
};

use crate::app::actions::SelectedAgentTask;
use crate::components::TaskListItem;

use super::types::TaskListDelegate as Delegate;

impl ListDelegate for Delegate {
    type Item = TaskListItem;

    fn sections_count(&self, _: &App) -> usize {
        self.industries.len()
    }

    fn items_count(&self, section: usize, _: &App) -> usize {
        // Return 0 items if the section is collapsed
        if self.is_section_collapsed(section) {
            0
        } else {
            self.matched_agent_tasks[section].len()
        }
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.prepare(query.to_owned());
        Task::ready(())
    }

    fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        println!("Confirmed with secondary: {}", secondary);
        window.dispatch_action(Box::new(SelectedAgentTask), cx);
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_section_header(
        &self,
        section: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement> {
        let Some(task_type) = self.industries.get(section) else {
            return None;
        };

        let is_collapsed = self.is_section_collapsed(section);
        let collapsed_sections = self.collapsed_sections.clone();
        let list_state = self.list_state.clone();

        // Use ChevronRight when collapsed, ChevronDown when expanded
        let chevron_icon = if is_collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };

        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .pb_1()
                .px_2()
                .gap_2()
                .text_sm()
                .rounded(cx.theme().radius)
                // Left side: collapsible section header
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .flex_1()
                        .text_color(cx.theme().muted_foreground)
                        .cursor_default()
                        .hover(|style| style.bg(cx.theme().secondary))
                        .rounded(cx.theme().radius)
                        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                            // Toggle the collapsed state
                            let mut collapsed = collapsed_sections.borrow_mut();
                            if collapsed.contains(&section) {
                                collapsed.remove(&section);
                            } else {
                                collapsed.insert(section);
                            }
                            drop(collapsed); // Release the borrow before updating

                            // Notify the list state to re-render
                            if let Some(list_state) = list_state.as_ref() {
                                _ = list_state.update(cx, |_, cx| {
                                    cx.notify();
                                });
                            }
                        })
                        .child(Icon::new(chevron_icon).size(px(14.)))
                        .child(Icon::new(IconName::Folder))
                        .child(task_type.clone()),
                )
                // Right side: add task button
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(20.))
                        .h(px(20.))
                        .rounded(px(4.))
                        .cursor_default()
                        .text_color(cx.theme().muted_foreground)
                        .hover(|style| {
                            style
                                .bg(cx.theme().accent)
                                .text_color(cx.theme().accent_foreground)
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _window, _cx| {
                            println!("Add new task to section: {}", section);
                            // TODO: Implement add task functionality
                        })
                        .child(Icon::new(IconName::Plus).size(px(14.))),
                ),
        )
    }

    fn render_item(&self, ix: IndexPath, _: &mut Window, _: &mut App) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index || Some(ix) == self.confirmed_index;
        if let Some(agent_task) = self.matched_agent_tasks[ix.section].get(ix.row) {
            return Some(TaskListItem::new(ix, agent_task.clone(), selected));
        }

        None
    }

    fn render_empty(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // Check if we have sections but all are collapsed
        let has_collapsed_sections = !self.industries.is_empty()
            && self.industries.len() == self.collapsed_sections.borrow().len();

        if has_collapsed_sections {
            // Render section headers so user can expand them
            let collapsed_sections = self.collapsed_sections.clone();
            let list_state = self.list_state.clone();

            v_flex()
                .w_full()
                .gap_1()
                .children(
                    self.industries
                        .iter()
                        .enumerate()
                        .map(|(section, task_type)| {
                            let collapsed_sections = collapsed_sections.clone();
                            let list_state = list_state.clone();
                            let task_type = task_type.clone();

                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .pb_1()
                                .px_2()
                                .gap_2()
                                .text_sm()
                                .rounded(cx.theme().radius)
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .flex_1()
                                        .text_color(cx.theme().muted_foreground)
                                        .cursor_default()
                                        .hover(|style| style.bg(cx.theme().secondary))
                                        .rounded(cx.theme().radius)
                                        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                            // Expand the section
                                            collapsed_sections.borrow_mut().remove(&section);

                                            if let Some(list_state) = list_state.as_ref() {
                                                _ = list_state.update(cx, |_, cx| {
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child(Icon::new(IconName::ChevronRight).size(px(14.)))
                                        .child(Icon::new(IconName::Folder))
                                        .child(task_type),
                                )
                        }),
                )
                .into_any_element()
        } else {
            // Default empty state
            h_flex()
                .size_full()
                .justify_center()
                .text_color(cx.theme().muted_foreground.opacity(0.6))
                .child(Icon::new(IconName::Inbox).size_12())
                .into_any_element()
        }
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn is_eof(&self, _: &App) -> bool {
        return !self.loading && !self.eof;
    }

    fn load_more_threshold(&self) -> usize {
        150
    }

    fn load_more(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        if !self.lazy_load {
            return;
        }

        cx.spawn_in(window, async move |view, window| {
            // Simulate network request, delay 1s to load data.
            Timer::after(Duration::from_secs(1)).await;

            _ = view.update_in(window, move |view, window, cx| {
                let query = view.delegate().query.clone();
                view.delegate_mut().extend_more(200);
                _ = view.delegate_mut().perform_search(&query, window, cx);
                view.delegate_mut().eof = view.delegate()._agent_tasks.len() >= 6000;
            });
        })
        .detach();
    }
}
