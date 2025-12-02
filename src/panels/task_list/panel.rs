use std::rc::Rc;

use gpui::{
    div, px, App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Render, Styled, Subscription, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    list::{List, ListEvent, ListState},
    popover::Popover,
    v_flex, ActiveTheme, Icon, IconName,
};

use agent_client_protocol_schema::{ContentBlock, SessionUpdate};

use crate::app::actions::{AddSessionToList, SelectedAgentTask};
use crate::task_schema::{AgentTask, TaskStatus};
use crate::utils;
use crate::{AppState, CreateTaskFromWelcome, ShowConversationPanel, ShowWelcomePanel};

use super::types::TaskListDelegate;

pub struct ListTaskPanel {
    focus_handle: FocusHandle,
    task_list: Entity<ListState<TaskListDelegate>>,
    selected_agent_task: Option<Rc<AgentTask>>,
    _subscriptions: Vec<Subscription>,
}

impl crate::panels::dock_panel::DockPanel for ListTaskPanel {
    fn title() -> &'static str {
        "Tasks"
    }

    fn description() -> &'static str {
        "A list displays a series of items."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
    fn paddings() -> Pixels {
        px(12.)
    }
}

impl ListTaskPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(window, cx));

        // Subscribe to session bus for all session updates
        Self::subscribe_to_session_updates(&entity, cx);

        entity
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut delegate = TaskListDelegate::new();
        delegate.load_all_tasks();

        let task_list = AppContext::new(cx, |cx| {
            ListState::new(delegate, window, cx).searchable(true)
        });

        // Set the weak reference to the list state in the delegate
        task_list.update(cx, |list, _| {
            list.delegate_mut().list_state = Some(task_list.downgrade());
        });

        let _subscriptions = vec![cx.subscribe_in(
            &task_list,
            window,
            |_this, _, ev: &ListEvent, window, cx| match ev {
                ListEvent::Select(ix) => {
                    println!("List Selected: {:?}", ix);
                    // Single click - show conversation panel
                    window.dispatch_action(Box::new(ShowConversationPanel), cx);
                }
                ListEvent::Confirm(ix) => {
                    println!("List Confirmed: {:?}", ix);
                    // Enter key - show conversation panel
                    window.dispatch_action(Box::new(ShowConversationPanel), cx);
                }
                ListEvent::Cancel => {
                    println!("List Cancelled");
                }
            },
        )];

        // Spawn a background task to randomly update task status for demo
        cx.spawn(async move |this, cx| {
            this.update(cx, |this, cx| {
                this.task_list.update(cx, |picker, _| {
                    picker
                        .delegate_mut()
                        ._agent_tasks
                        .iter_mut()
                        .for_each(|agent_task| {
                            // Clone the task and update its status
                            let mut updated_task = (**agent_task).clone();
                            updated_task.status = crate::task_data::random_status();
                            *agent_task = Rc::new(updated_task.prepare());
                        });
                    picker.delegate_mut().prepare("");
                });
                cx.notify();
            })
            .ok();
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            task_list,
            selected_agent_task: None,
            _subscriptions,
        }
    }

    /// Subscribe to session bus to update task subtitles with message previews
    fn subscribe_to_session_updates(entity: &Entity<Self>, cx: &mut App) {
        let weak_entity = entity.downgrade();
        let session_bus = AppState::global(cx).session_bus.clone();

        // Create channel for cross-thread communication
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, SessionUpdate)>();

        // Subscribe to all session updates
        session_bus.subscribe(move |event| {
            let _ = tx.send((event.session_id.clone(), (*event.update).clone()));
        });

        // Spawn background task to receive updates and update task subtitles
        cx.spawn(async move |cx| {
            while let Some((session_id, update)) = rx.recv().await {
                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.handle_session_update(session_id, update, cx);
                        });
                    }
                });
            }
        })
        .detach();

        log::info!("ListTaskPanel subscribed to session bus");
    }

    /// Handle session updates and update task subtitles
    fn handle_session_update(
        &mut self,
        session_id: String,
        update: SessionUpdate,
        cx: &mut Context<Self>,
    ) {
        // Extract text from the update
        let text = match &update {
            SessionUpdate::UserMessageChunk(chunk) => {
                log::debug!("User message chunk: {:?}", chunk);
                Self::extract_text_from_content(&chunk.content)
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                log::debug!("Agent message chunk: {:?}", chunk);
                Self::extract_text_from_content(&chunk.content)
            }
            _ => {
                log::debug!("Ignoring session update: {:?}", update);
                return;
            } // Ignore other update types
        };

        if text.is_empty() {
            return;
        }

        // Update the task with matching session_id
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            let mut found = false;

            // Find and update the task
            for task in delegate._agent_tasks.iter_mut() {
                if task.session_id.as_ref() == Some(&session_id) {
                    let mut updated_task = (**task).clone();
                    // Truncate text to ~50 characters for subtitle
                    let preview = if text.len() > 50 {
                        format!("{}...", &text[..50])
                    } else {
                        text.clone()
                    };
                    updated_task.update_subtitle(preview);
                    *task = Rc::new(updated_task);
                    found = true;
                    break;
                }
            }

            if found {
                delegate.prepare("");
                cx.notify();
                log::debug!("Updated task subtitle for session: {}", session_id);
            }
        });
    }

    /// Extract text from ContentBlock
    fn extract_text_from_content(content: &ContentBlock) -> String {
        match content {
            ContentBlock::Text(text_content) => text_content.text.clone(),
            _ => String::new(),
        }
    }

    fn selected_agent_task(
        &mut self,
        _: &SelectedAgentTask,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = self.task_list.read(cx);
        if let Some(agent_task) = picker.delegate().selected_agent_task() {
            log::debug!("Selected agent task: {:?}", &agent_task.name);
            self.selected_agent_task = Some(agent_task);
        }
    }

    /// Handle action to create a new task from the welcome panel
    fn on_create_task_from_welcome(
        &mut self,
        action: &CreateTaskFromWelcome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task_name = action.task_input.clone();
        log::debug!("Creating new task from welcome: {:?}", action);
        // Create a new task with InProgress status
        let new_task = AgentTask {
            name: task_name,
            task_type: "Conversation".to_string(),
            add_new_code_lines: 0,
            delete_code_lines: 0,
            status: TaskStatus::InProgress,
            session_id: None,
            subtitle: None,
            change_timestamp: 0,
            change_timestamp_str: "".into(),
            add_new_code_lines_str: "+0".into(),
            delete_code_lines_str: "-0".into(),
        }
        .prepare();

        // Add task to the beginning of the list
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            delegate._agent_tasks.insert(0, Rc::new(new_task));
            delegate.prepare("");
            cx.notify();
        });
    }

    /// Handle action to add a new session to the list
    fn on_add_session_to_list(
        &mut self,
        action: &AddSessionToList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::info!(
            "Received AddSessionToList action: session_id={}, task_name={}",
            action.session_id,
            action.task_name
        );

        let task_name = action.task_name.clone();
        let session_id = action.session_id.clone();

        // Create a new task for this session
        let new_task = AgentTask::new_for_session(task_name, session_id.clone());

        // Add task to the beginning of the list in the "Default" section
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            delegate._agent_tasks.insert(0, Rc::new(new_task));
            delegate.prepare("");
            cx.notify();
        });

        log::info!("Added session to list: {}", session_id);
    }

    /// Handle click on "New Task" button - shows the welcome panel
    fn on_new_task_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Ensure this panel has focus before dispatching action
        log::debug!("Focusing on 'New Task' button");
        window.focus(&self.focus_handle);
        window.dispatch_action(Box::new(ShowWelcomePanel), cx);
    }
}

impl Focusable for ListTaskPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ListTaskPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .child(
                Button::new("btn-new-task")
                    .label("New Task")
                    .primary()
                    .icon(Icon::new(IconName::Plus))
                    .on_click(cx.listener(Self::on_new_task_click)),
            )
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::selected_agent_task))
            .on_action(cx.listener(Self::on_create_task_from_welcome))
            .on_action(cx.listener(Self::on_add_session_to_list))
            .size_full()
            .gap_4()
            .child(
                List::new(&self.task_list)
                    .p(px(8.))
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius),
            )
            // Bottom action buttons with popover
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        Popover::new("add-repository-popover")
                            .trigger(
                                Button::new("btn-add-repository")
                                    .label("Add repository")
                                    .icon(Icon::new(IconName::Plus))
                                    .ghost(),
                            )
                            .content(|_state, _window, cx| {
                                let popover_entity = cx.entity();
                                v_flex()
                                    .gap_1()
                                    .min_w(px(200.))
                                    .child(
                                        // Open project button
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .px_3()
                                            .py_2()
                                            .rounded(cx.theme().radius)
                                            .cursor_default()
                                            .hover(|style| style.bg(cx.theme().secondary))
                                            .on_mouse_down(MouseButton::Left, {
                                                let popover = popover_entity.clone();
                                                move |_, window, cx| {
                                                    // Close the popover first
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });

                                                    // Then spawn the folder picker
                                                    cx.spawn(async move |_cx| {
                                                        utils::pick_and_log_folder(
                                                            "Select Project Folder",
                                                            "Task List",
                                                        )
                                                        .await;
                                                    })
                                                    .detach();
                                                }
                                            })
                                            .child(Icon::new(IconName::Folder).size(px(16.)))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().foreground)
                                                    .child("Open project"),
                                            ),
                                    )
                                    .child(
                                        // Clone from URL button
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .px_3()
                                            .py_2()
                                            .rounded(cx.theme().radius)
                                            .cursor_default()
                                            .hover(|style| style.bg(cx.theme().secondary))
                                            .on_mouse_down(MouseButton::Left, {
                                                let popover = popover_entity.clone();
                                                move |_, window, cx| {
                                                    // Close the popover
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });

                                                    println!("Clone from URL clicked");
                                                    // TODO: Implement clone from URL functionality
                                                }
                                            })
                                            .child(Icon::new(IconName::Globe).size(px(16.)))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().foreground)
                                                    .child("Clone from URL"),
                                            ),
                                    )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("btn-notifications")
                                    .icon(Icon::new(IconName::Bell))
                                    .ghost()
                                    .on_click(|_, _, _| {
                                        println!("Notifications clicked");
                                        // TODO: Implement notifications functionality
                                    }),
                            )
                            .child(
                                Button::new("btn-settings")
                                    .icon(Icon::new(IconName::Settings))
                                    .ghost()
                                    .on_click(|_, _, _| {
                                        println!("Settings clicked");
                                        // TODO: Implement settings functionality
                                    }),
                            ),
                    ),
            )
    }
}
