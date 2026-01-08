use gpui::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use agent_client_protocol::{self as acp, ToolCall, ToolCallContent, ToolCallStatus};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex,
};
use similar::{ChangeTag, TextDiff};

use crate::components::{StatusIndicator, DiffView};
use crate::core::services::SessionStatus;
use crate::panels::conversation::types::{ToolCallStatusExt, ToolKindExt};

/// Diff statistics
#[derive(Debug, Clone, Default)]
struct DiffStats {
    additions: usize,
    deletions: usize,
}

/// Calculate diff statistics from old and new text
fn calculate_diff_stats(old_text: &str, new_text: &str) -> DiffStats {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut additions = 0;
    let mut deletions = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }

    DiffStats {
        additions,
        deletions,
    }
}

/// Extract diff statistics from tool call content
fn extract_diff_stats_from_tool_call(tool_call: &ToolCall) -> Option<DiffStats> {
    // Find the first Diff content in the tool call
    for content in &tool_call.content {
        if let ToolCallContent::Diff(diff) = content {
            return Some(match &diff.old_text {
                Some(old_text) => calculate_diff_stats(old_text, &diff.new_text),
                None => {
                    // New file - all lines are additions
                    DiffStats {
                        additions: diff.new_text.lines().count(),
                        deletions: 0,
                    }
                }
            });
        }
    }
    None
}

/// Tool call item component based on ACP's ToolCall - stateful version
pub struct ToolCallItem {
    tool_call: ToolCall,
    open: bool,
}

impl ToolCallItem {
    pub fn new(tool_call: ToolCall) -> Self {
        Self {
            tool_call,
            open: false,
        }
    }

    /// Toggle the open state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    /// Set the open state
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        cx.notify();
    }
    // pub fn update_tool_title(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
    //     self.tool_call.title = title;
    //     cx.notify();
    // }
    /// Update the tool call data
    pub fn update_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
        log::debug!("tool_call: {:?}", &tool_call);
        self.tool_call = tool_call;
        cx.notify();
    }

    /// Update the status
    pub fn update_status(&mut self, status: ToolCallStatus, cx: &mut Context<Self>) {
        self.tool_call.status = status;
        cx.notify();
    }

    /// Add content to the tool call
    pub fn add_content(&mut self, content: ToolCallContent, cx: &mut Context<Self>) {
        self.tool_call.content.push(content);
        cx.notify();
    }

    fn has_content(&self) -> bool {
        !self.tool_call.content.is_empty()
    }

    /// Render content based on type
    fn render_content(&self, content: &ToolCallContent, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match content {
            ToolCallContent::Diff(diff) => {
                // Use DiffView component for diff content, limited to 10 lines
                let diff_view = DiffView::new(diff.clone())
                    .max_lines(10)
                    .context_lines(3)
                    .show_file_header(false);  // Hide file header in compact view

                diff_view.render(window, &mut **cx).into_any_element()
            }
            ToolCallContent::Content(c) => match &c.content {
                acp::ContentBlock::Text(text) => {
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .line_height(px(18.))
                        .child(text.text.clone())
                        .into_any_element()
                }
                _ => div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child("Unsupported content type")
                    .into_any_element(),
            },
            ToolCallContent::Terminal(terminal) => {
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .line_height(px(18.))
                    .child(format!("Terminal: {}", terminal.terminal_id))
                    .into_any_element()
            }
            _ => div()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .child("Unknown content type")
                .into_any_element(),
        }
    }

    /// Convert ToolCallStatus to SessionStatus for StatusIndicator
    fn status_to_session_status(&self) -> SessionStatus {
        match self.tool_call.status {
            ToolCallStatus::Pending => SessionStatus::Pending,
            ToolCallStatus::InProgress => SessionStatus::InProgress,
            ToolCallStatus::Completed => SessionStatus::Completed,
            ToolCallStatus::Failed => SessionStatus::Failed,
            _ => SessionStatus::Idle,
        }
    }
}

impl Render for ToolCallItem {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_content = self.has_content();
        let status_color = match self.tool_call.status {
            ToolCallStatus::Completed => cx.theme().green,
            ToolCallStatus::Failed => cx.theme().red,
            ToolCallStatus::InProgress => cx.theme().accent,
            ToolCallStatus::Pending | _ => cx.theme().muted_foreground,
        };

        let open = self.open;
        let tool_call_id = self.tool_call.tool_call_id.clone();

        // Extract diff stats if this is a diff tool call
        let diff_stats = extract_diff_stats_from_tool_call(&self.tool_call);

        Collapsible::new()
            .open(open)
            .w_full()
            .gap_2()
            // Header - always visible
            .child(
                h_flex()
                    .items_start()
                    .gap_2()
                    .child(
                        // Kind icon
                        StatusIndicator::new(self.status_to_session_status()).size(12.0),
                    )
                    .child(
                        // Title
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .child(self.tool_call.title.clone()),
                    )
                    // Show diff stats if available
                    .when_some(diff_stats, |this, stats| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .child(
                                    // Additions
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(cx.theme().green)
                                        .child(format!("+{}", stats.additions)),
                                )
                                .child(
                                    // Deletions
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(cx.theme().red)
                                        .child(format!("-{}", stats.deletions)),
                                ),
                        )
                    })
                    .child(
                        // Status icon
                        self.tool_call
                            .status
                            .icon()
                            .size(px(14.))
                            .text_color(status_color),
                    )
                    .when(has_content, |this| {
                        // Add expand/collapse button only if there's content
                        this.child(
                            Button::new(SharedString::from(format!(
                                "tool-call-{}-toggle",
                                tool_call_id
                            )))
                            .icon(if open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(
                                |this, _ev, _window, cx| {
                                    this.toggle(cx);
                                },
                            )),
                        )
                    }),
            )
            // Content - only visible when open and has content
            .when(has_content, |this| {
                this.content(
                    v_flex()
                        .gap_2()
                        .p_3()
                        .pl_8()
                        .children(
                            self.tool_call
                                .content
                                .iter()
                                .map(|content| self.render_content(content, window, cx)),
                        ),
                )
            })
    }
}

/// A stateful wrapper for ToolCallItem that can be used as a GPUI view
pub struct ToolCallItemView {
    item: Entity<ToolCallItem>,
}

impl ToolCallItemView {
    pub fn new(tool_call: ToolCall, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let item = cx.new(|_| ToolCallItem::new(tool_call));
            Self { item }
        })
    }

    /// Update the tool call data
    pub fn update_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.update_tool_call(tool_call, cx);
        });
        cx.notify();
    }

    /// Update the status
    pub fn update_status(&mut self, status: ToolCallStatus, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.update_status(status, cx);
        });
        cx.notify();
    }

    /// Add content to the tool call
    pub fn add_content(&mut self, content: ToolCallContent, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.add_content(content, cx);
        });
        cx.notify();
    }

    /// Set content for the tool call
    pub fn set_content(&mut self, content: Vec<ToolCallContent>, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.tool_call.content = content;
            cx.notify();
        });
        cx.notify();
    }

    /// Toggle the open state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.toggle(cx);
        });
        cx.notify();
    }

    /// Set the open state
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.set_open(open, cx);
        });
        cx.notify();
    }
}

impl Render for ToolCallItemView {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.item.clone()
    }
}
