use agent_client_protocol::{ContentBlock, ToolCallStatus, ToolKind};
use gpui::SharedString;
use gpui_component::{Icon, IconName};

// ============================================================================
// Helper Traits
// ============================================================================

/// Helper trait to get icon for ToolKind
pub trait ToolKindExt {
    fn icon(&self) -> Icon;
}

impl ToolKindExt for ToolKind {
    fn icon(&self) -> Icon {
        match self {
            ToolKind::Read => Icon::new(IconName::Eye),
            ToolKind::Edit => Icon::new(IconName::Replace),
            ToolKind::Delete => Icon::new(IconName::Delete),
            ToolKind::Move => Icon::new(IconName::ArrowRight),
            ToolKind::Search => Icon::new(IconName::Search),
            ToolKind::Execute => Icon::new(IconName::SquareTerminal),
            ToolKind::Think => Icon::new(crate::assets::Icon::Brain),
            ToolKind::Fetch => Icon::new(IconName::Globe),
            ToolKind::SwitchMode => Icon::new(IconName::ArrowRight),
            ToolKind::Other | _ => Icon::new(IconName::Ellipsis),
        }
    }
}

pub trait ToolCallStatusExt {
    fn icon(&self) -> Icon;
}

impl ToolCallStatusExt for ToolCallStatus {
    fn icon(&self) -> Icon {
        match self {
            ToolCallStatus::Pending => Icon::new(IconName::Dash),
            ToolCallStatus::InProgress => Icon::new(IconName::Dash),
            ToolCallStatus::Completed => Icon::new(IconName::CircleCheck),
            ToolCallStatus::Failed => Icon::new(IconName::CircleX),
            _ => Icon::new(IconName::Dash),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

pub fn extract_filename(uri: &str) -> String {
    uri.split('/').next_back().unwrap_or("unknown").to_string()
}

pub fn get_file_icon(mime_type: &Option<String>) -> IconName {
    if let Some(mime) = mime_type {
        if mime.contains("python")
            || mime.contains("javascript")
            || mime.contains("typescript")
            || mime.contains("rust")
            || mime.contains("json")
        {
            return IconName::File;
        }
    }
    IconName::File
}

// ============================================================================
// Resource Info Structure
// ============================================================================

#[derive(Clone)]
pub struct ResourceInfo {
    pub uri: SharedString,
    pub name: SharedString,
    pub mime_type: Option<SharedString>,
    pub text: Option<SharedString>,
}

impl ResourceInfo {
    pub fn from_content_block(content: &ContentBlock) -> Option<Self> {
        match content {
            ContentBlock::ResourceLink(link) => Some(ResourceInfo {
                uri: link.uri.clone().into(),
                name: link.name.clone().into(),
                mime_type: link.mime_type.clone().map(Into::into),
                text: None,
            }),
            // TODO: Handle Resource type when schema is clarified
            // ContentBlock::Resource(res) => { ... }
            _ => None,
        }
    }
}
