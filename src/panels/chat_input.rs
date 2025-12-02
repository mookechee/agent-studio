use gpui::{
    px, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Pixels, Render, Styled, Subscription, Window,
};

use gpui_component::{
    input::InputState,
    list::{ListDelegate, ListItem, ListState},
    select::SelectState,
    v_flex, ActiveTheme, IndexPath,
};

use crate::components::ChatInputBox;
use crate::AppState;

/// Delegate for the context list in the chat input popover
struct ContextListDelegate {
    items: Vec<ContextItem>,
}

#[derive(Clone)]
struct ContextItem {
    name: &'static str,
    icon: &'static str,
}

impl ContextListDelegate {
    fn new() -> Self {
        Self {
            items: vec![
                ContextItem {
                    name: "Files",
                    icon: "file",
                },
                ContextItem {
                    name: "Folders",
                    icon: "folder",
                },
                ContextItem {
                    name: "Code",
                    icon: "code",
                },
                ContextItem {
                    name: "Git Changes",
                    icon: "git-branch",
                },
                ContextItem {
                    name: "Terminal",
                    icon: "terminal",
                },
                ContextItem {
                    name: "Problems",
                    icon: "alert-circle",
                },
                ContextItem {
                    name: "URLs",
                    icon: "link",
                },
            ],
        }
    }
}

impl ListDelegate for ContextListDelegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.items.len()
    }

    fn render_item(&self, ix: IndexPath, _: &mut Window, _: &mut App) -> Option<Self::Item> {
        let item = self.items.get(ix.row)?;
        Some(ListItem::new(ix).child(item.name))
    }

    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
    }

    fn confirm(&mut self, _: bool, _: &mut Window, _cx: &mut Context<ListState<Self>>) {
        // Handle item selection - for now just close the popover
    }

    fn cancel(&mut self, _: &mut Window, _cx: &mut Context<ListState<Self>>) {
        // Close the popover on cancel
    }
}

pub struct ChatInputPanel {
    focus_handle: FocusHandle,
    input_state: Entity<InputState>,
    context_list: Entity<ListState<ContextListDelegate>>,
    context_popover_open: bool,
    mode_select: Entity<SelectState<Vec<&'static str>>>,
    agent_select: Entity<SelectState<Vec<String>>>,
    session_select: Entity<SelectState<Vec<String>>>,
    current_session_id: Option<String>,
    has_agents: bool,
    _subscriptions: Vec<Subscription>,
}

impl crate::panels::dock_panel::DockPanel for ChatInputPanel {
    fn title() -> &'static str {
        "Chat Input"
    }

    fn description() -> &'static str {
        "A chat input box for sending messages."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
    fn paddings() -> Pixels {
        px(0.)
    }
}

impl ChatInputPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(window, cx));

        // Subscribe to agent_select focus to refresh agents list when no agents available
        entity.update(cx, |this, cx| {
            let agent_select_focus = this.agent_select.focus_handle(cx);
            let subscription = cx.on_focus(
                &agent_select_focus,
                window,
                |this: &mut Self, window, cx| {
                    this.try_refresh_agents(window, cx);
                },
            );
            this._subscriptions.push(subscription);
        });

        entity
    }

    fn new(window: &mut Window, cx: &mut App) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(2, 8) // Auto-grow from 2 to 8 rows
                .soft_wrap(true) // Enable word wrapping
                .placeholder("Ask, search, or make anything...")
        });

        let context_list =
            cx.new(|cx| ListState::new(ContextListDelegate::new(), window, cx).searchable(true));

        let mode_select = cx.new(|cx| {
            SelectState::new(
                vec!["Auto", "Ask", "Plan", "Code", "Explain"],
                Some(IndexPath::default()), // Select "Auto" by default
                window,
                cx,
            )
        });

        // Get available agents from AppState
        let agents = AppState::global(cx)
            .agent_manager()
            .map(|m| m.list_agents())
            .unwrap_or_default();

        let has_agents = !agents.is_empty();

        // Default to first agent if available
        let default_agent = if has_agents {
            Some(IndexPath::default())
        } else {
            None
        };

        // Use placeholder if no agents available
        let agent_list = if has_agents {
            agents
        } else {
            vec!["No agents".to_string()]
        };

        let agent_select = cx.new(|cx| SelectState::new(agent_list, default_agent, window, cx));

        // Initialize session selector (initially empty)
        let session_select = cx.new(|cx| {
            SelectState::new(
                vec!["No sessions".to_string()],
                None,
                window,
                cx,
            )
        });

        Self {
            focus_handle: cx.focus_handle(),
            input_state,
            context_list,
            context_popover_open: false,
            mode_select,
            agent_select,
            session_select,
            current_session_id: None,
            has_agents,
            _subscriptions: Vec::new(),
        }
    }

    /// Try to refresh agents list from AppState if we don't have agents yet
    fn try_refresh_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_agents {
            return;
        }

        let agents = AppState::global(cx)
            .agent_manager()
            .map(|m| m.list_agents())
            .unwrap_or_default();

        if agents.is_empty() {
            return;
        }

        // We now have agents, update the select
        self.has_agents = true;
        self.agent_select.update(cx, |state, cx| {
            state.set_items(agents, window, cx);
            state.set_selected_index(Some(IndexPath::default()), window, cx);
        });
        cx.notify();
    }

    /// Refresh sessions for the currently selected agent
    fn refresh_sessions_for_agent(&mut self, agent_name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let sessions = agent_service.list_sessions_for_agent(agent_name);

        if sessions.is_empty() {
            // No sessions for this agent
            self.session_select.update(cx, |state, cx| {
                state.set_items(vec!["No sessions".to_string()], window, cx);
                state.set_selected_index(None, window, cx);
            });
            self.current_session_id = None;
        } else {
            // Display sessions (show first 8 chars of session ID)
            let session_display: Vec<String> = sessions
                .iter()
                .map(|s| {
                    let short_id = if s.session_id.len() > 8 {
                        &s.session_id[..8]
                    } else {
                        &s.session_id
                    };
                    format!("Session {}", short_id)
                })
                .collect();

            self.session_select.update(cx, |state, cx| {
                state.set_items(session_display, window, cx);
                state.set_selected_index(Some(IndexPath::default()), window, cx);
            });

            // Set current session to the first one
            self.current_session_id = sessions.first().map(|s| s.session_id.clone());
        }

        cx.notify();
    }

    /// Create a new session for the currently selected agent
    fn create_new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != "No agents" => name,
            _ => return,
        };

        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let weak_self = cx.entity().downgrade();
        cx.spawn_in(window, async move |_this, window| {
            match agent_service.create_session(&agent_name).await {
                Ok(session_id) => {
                    log::info!("[ChatInputPanel] Created new session: {}", session_id);
                    _ = window.update(|window, cx| {
                        if let Some(this) = weak_self.upgrade() {
                            this.update(cx, |this, cx| {
                                this.current_session_id = Some(session_id.clone());
                                this.refresh_sessions_for_agent(&agent_name, window, cx);
                            });
                        }
                    });
                }
                Err(e) => {
                    log::error!("[ChatInputPanel] Failed to create session: {}", e);
                }
            }
        }).detach();
    }

    /// Send message to the selected agent using MessageService
    fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Get the selected agent name
        let agent_name = self.agent_select.read(cx).selected_value().cloned();

        let agent_name = match agent_name {
            Some(name) if name != "No agents" => name,
            _ => {
                log::warn!("[ChatInputPanel] No agent selected");
                return;
            }
        };

        // Get the input text
        let input_text = self.input_state.read(cx).value().to_string();
        if input_text.trim().is_empty() {
            log::info!("[ChatInputPanel] Skipping send: input is empty.");
            return;
        }
        log::info!("[ChatInputPanel] Sending message: \"{}\"", input_text);

        // Get MessageService
        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("[ChatInputPanel] MessageService not initialized");
                return;
            }
        };

        // Get AgentService
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("[ChatInputPanel] AgentService not initialized");
                return;
            }
        };

        // Clear the input immediately
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });

        // Check if we have a current session
        let session_id = if let Some(sid) = &self.current_session_id {
            sid.clone()
        } else {
            // No session selected, create new one and send message after creation
            log::info!("[ChatInputPanel] No session selected, creating new session");
            let weak_self = cx.entity().downgrade();
            let agent_name_for_spawn = agent_name.clone();

            cx.spawn_in(window, async move |_this, window| {
                match agent_service.create_session(&agent_name_for_spawn).await {
                    Ok(new_session_id) => {
                        log::info!("[ChatInputPanel] Created session {} for agent {}", new_session_id, agent_name_for_spawn);

                        // Update UI with new session
                        _ = window.update(|window, cx| {
                            if let Some(this) = weak_self.upgrade() {
                                this.update(cx, |this, cx| {
                                    this.current_session_id = Some(new_session_id.clone());
                                    this.refresh_sessions_for_agent(&agent_name_for_spawn, window, cx);
                                });
                            }
                        });

                        // Send the message to the new session
                        match message_service.send_message_to_session(&agent_name_for_spawn, &new_session_id, input_text).await {
                            Ok(_) => {
                                log::info!("[ChatInputPanel] Message sent successfully to new session {}", new_session_id);
                            }
                            Err(e) => {
                                log::error!("[ChatInputPanel] Failed to send message: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("[ChatInputPanel] Failed to create session: {}", e);
                    }
                }
            }).detach();
            return;
        };

        // We have a session, send message directly
        cx.spawn(async move |_this, _cx| {
            match message_service.send_message_to_session(&agent_name, &session_id, input_text).await {
                Ok(_) => {
                    log::info!("[ChatInputPanel] Message sent successfully to session {}", session_id);
                }
                Err(e) => {
                    log::error!("[ChatInputPanel] Failed to send message: {}", e);
                }
            }
        }).detach();
    }
}

impl Focusable for ChatInputPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatInputPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .justify_end()
            .bg(cx.theme().background)
            .child(
                ChatInputBox::new("chat-input-box", self.input_state.clone())
                    // .title("Send a message")
                    .context_list(self.context_list.clone(), cx)
                    .context_popover_open(self.context_popover_open)
                    .on_context_popover_change(cx.listener(|this, open: &bool, _, cx| {
                        this.context_popover_open = *open;
                        cx.notify();
                    }))
                    .on_send(cx.listener(|this, _, window, cx| {
                        this.send_message(window, cx);
                    }))
                    .mode_select(self.mode_select.clone())
                    .agent_select(self.agent_select.clone())
                    .session_select(self.session_select.clone())
                    .on_new_session(cx.listener(|this, _, window, cx| {
                        this.create_new_session(window, cx);
                    })),
            )
    }
}
