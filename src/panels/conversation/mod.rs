// Conversation panel module - modularized for better maintainability

mod components;
mod helpers;
mod panel;
mod rendered_item;
pub mod types;

// Re-export public API
pub use panel::ConversationPanel;
