// Event bus modules
pub mod permission_bus;
pub mod session_bus;
pub mod workspace_bus;

// Re-export event bus types
pub use permission_bus::{PermissionBusContainer, PermissionRequestEvent};
pub use session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};
pub use workspace_bus::{WorkspaceUpdateBusContainer, WorkspaceUpdateEvent};
