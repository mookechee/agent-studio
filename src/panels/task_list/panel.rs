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
use crate::schemas::workspace_schema::WorkspaceTask;
use crate::{AppState, CreateTaskFromWelcome, ShowConversationPanel, ShowWelcomePanel};

use super::types::TaskListDelegate;

pub struct ListTaskPanel {
    focus_handle: FocusHandle,
    task_list: Entity<ListState<TaskListDelegate>>,
    selected_task: Option<Rc<WorkspaceTask>>,
    _subscriptions: Vec<Subscription>,
}

impl crate::panels::dock_panel::DockPanel for ListTaskPanel {
    fn title() -> &'static str {
        "Tasks"
    }

    fn description() -> &'static str {
        "Project-based task list"
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

        // Subscribe to workspace bus for workspace updates
        Self::subscribe_to_workspace_updates(&entity, cx);

        // Load initial workspace data
        Self::load_workspace_data(&entity, cx);

        entity
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = TaskListDelegate::new();

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
                    println!("Task Selected: {:?}", ix);
                    // Single click - show conversation panel
                    window.dispatch_action(Box::new(ShowConversationPanel), cx);
                }
                ListEvent::Confirm(ix) => {
                    println!("Task Confirmed: {:?}", ix);
                    // Enter key - show conversation panel
                    window.dispatch_action(Box::new(ShowConversationPanel), cx);
                }
                ListEvent::Cancel => {
                    println!("List Cancelled");
                }
            },
        )];

        Self {
            focus_handle: cx.focus_handle(),
            task_list,
            selected_task: None,
            _subscriptions,
        }
    }

    /// Load workspace data from WorkspaceService
    fn load_workspace_data(entity: &Entity<Self>, cx: &mut App) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("WorkspaceService not initialized");
                return;
            }
        };

        let weak_entity = entity.downgrade();
        cx.spawn(async move |cx| {
            // Load workspaces and tasks
            let workspaces = workspace_service.list_workspaces().await;
            let all_tasks = workspace_service.get_all_tasks().await;

            log::info!(
                "Loaded {} workspaces and {} tasks",
                workspaces.len(),
                all_tasks.len()
            );

            // Update the UI
            _ = cx.update(|cx| {
                if let Some(entity) = weak_entity.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.task_list.update(cx, |list, cx| {
                            list.delegate_mut().load_from_service(workspaces, all_tasks);
                            cx.notify();
                        });
                    });
                }
            });
        })
        .detach();
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
                log::debug!("User message chunk for session {}", session_id);
                Self::extract_text_from_content(&chunk.content)
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                log::debug!("Agent message chunk for session {}", session_id);
                Self::extract_text_from_content(&chunk.content)
            }
            _ => {
                log::debug!("Ignoring session update type");
                return;
            }
        };

        if text.is_empty() {
            return;
        }

        // Truncate text to ~50 characters for preview
        let preview = if text.len() > 50 {
            format!("{}...", &text[..50])
        } else {
            text.clone()
        };

        // Update the task with matching session_id
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            if delegate.update_task_message(&session_id, preview) {
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
        if let Some(task) = picker.delegate().selected_task() {
            log::debug!("Selected task: {:?}", &task.name);
            self.selected_task = Some(task);
        }
    }

    /// Handle action to create a new task from the welcome panel
    /// Note: Task creation is handled by workspace/actions.rs
    /// This handler waits a bit and then reloads the data to display the newly created task
    fn on_create_task_from_welcome(
        &mut self,
        _action: &CreateTaskFromWelcome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::debug!("[ListTaskPanel] Scheduling workspace data reload after task creation");

        // Wait for the task to be created (workspace/actions.rs is async)
        // then reload workspace data to show the newly created task
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            // Wait 500ms for the task creation to complete
            smol::Timer::after(std::time::Duration::from_millis(500)).await;

            // Reload workspace data
            _ = cx.update(|cx| {
                if let Some(this) = entity.upgrade() {
                    Self::load_workspace_data(&this, cx);
                }
            });
        })
        .detach();
    }

    /// Handle action to add a new session to the list
    fn on_add_session_to_list(
        &mut self,
        action: &AddSessionToList,
        _: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        log::info!(
            "Received AddSessionToList action: session_id={}, task_name={}",
            action.session_id,
            action.task_name
        );

        // In the new workspace system, tasks are created via CreateTaskFromWelcome
        // This action might be deprecated, but we'll log it for now
        log::warn!("AddSessionToList is deprecated in workspace-based system");
    }

    /// Handle click on "New Task" button - shows the welcome panel
    fn on_new_task_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::debug!("Focusing on 'New Task' button");
        window.focus(&self.focus_handle);
        window.dispatch_action(Box::new(ShowWelcomePanel), cx);
    }

    /// Subscribe to workspace bus for workspace updates (task created, workspace added, etc.)
    fn subscribe_to_workspace_updates(entity: &Entity<Self>, cx: &mut App) {
        let weak_entity = entity.downgrade();
        let workspace_bus = AppState::global(cx).workspace_bus.clone();

        // Create channel for cross-thread communication
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // Subscribe to workspace updates
        workspace_bus.lock().unwrap().subscribe(move |event| {
            let _ = tx.send(event.clone());
        });

        // Spawn background task to receive updates and reload data
        cx.spawn(async move |cx| {
            while let Some(event) = rx.recv().await {
                log::info!("[ListTaskPanel] Received workspace update: {:?}", event);

                // Small delay to ensure data is written to disk
                smol::Timer::after(std::time::Duration::from_millis(100)).await;

                // Reload workspace data
                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        Self::load_workspace_data(&entity, cx);
                    }
                });
            }
        })
        .detach();

        log::info!("[ListTaskPanel] Subscribed to workspace bus");
    }

    /// Handle "Open project" click - add a workspace
    fn on_open_project_click(&self, window: &mut Window, cx: &mut Context<Self>) {
        log::debug!("Open project clicked");

        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("WorkspaceService not initialized");
                return;
            }
        };

        let weak_self = cx.entity().downgrade();
        cx.spawn_in(window, async move |_this, _window| {
            // Use the file picker to select a folder
            match rfd::AsyncFileDialog::new()
                .set_title("Select Project Folder")
                .pick_folder()
                .await
            {
                Some(folder) => {
                    let path = folder.path().to_path_buf();
                    log::info!("Selected folder: {:?}", path);

                    // Add workspace
                    match workspace_service.add_workspace(path).await {
                        Ok(workspace) => {
                            log::info!("Added workspace: {}", workspace.name);

                            // Reload workspace data
                            _ = _window.update(|_window, cx| {
                                if let Some(this) = weak_self.upgrade() {
                                    Self::load_workspace_data(&this, cx);
                                }
                            });
                        }
                        Err(e) => {
                            log::error!("Failed to add workspace: {}", e);
                        }
                    }
                }
                None => {
                    log::debug!("Folder selection cancelled");
                }
            }
        })
        .detach();
    }
}

impl Focusable for ListTaskPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ListTaskPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel_weak = cx.entity().downgrade();

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
                            .content(move |_state, _window, cx| {
                                let popover_entity = cx.entity();
                                let panel_weak_clone = panel_weak.clone();

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
                                                let panel_weak = panel_weak_clone.clone();
                                                move |_, window, cx| {
                                                    // Close the popover first
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });

                                                    // Open project
                                                    if let Some(panel) = panel_weak.upgrade() {
                                                        panel.update(cx, |this, cx| {
                                                            this.on_open_project_click(window, cx);
                                                        });
                                                    }
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
                                        // Clone from URL button (placeholder for future)
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

                                                    log::info!("Clone from URL clicked");
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
                                        log::info!("Notifications clicked");
                                        // TODO: Implement notifications functionality
                                    }),
                            )
                            .child(
                                Button::new("btn-settings")
                                    .icon(Icon::new(IconName::Settings))
                                    .ghost()
                                    .on_click(|_, _, _| {
                                        log::info!("Settings clicked");
                                        // TODO: Implement settings functionality
                                    }),
                            ),
                    ),
            )
    }
}
