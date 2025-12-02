use gpui::{App, AppContext, Entity, Global, SharedString};
use std::sync::Arc;

use crate::{
    core::agent::{AgentManager, PermissionStore},
    core::event_bus::{
        PermissionBusContainer, SessionUpdateBusContainer, WorkspaceUpdateBusContainer,
    },
    core::services::{AgentService, MessageService, WorkspaceService},
};

/// Welcome session info - stores the session created when user selects an agent
#[derive(Clone, Debug)]
pub struct WelcomeSession {
    pub session_id: String,
    pub agent_name: String,
}

pub struct AppState {
    pub invisible_panels: Entity<Vec<SharedString>>,
    agent_manager: Option<Arc<AgentManager>>,
    permission_store: Option<Arc<PermissionStore>>,
    pub session_bus: SessionUpdateBusContainer,
    pub permission_bus: PermissionBusContainer,
    pub workspace_bus: WorkspaceUpdateBusContainer,
    /// Current welcome session - created when user selects an agent
    welcome_session: Option<WelcomeSession>,
    /// Service layer
    agent_service: Option<Arc<AgentService>>,
    message_service: Option<Arc<MessageService>>,
    workspace_service: Option<Arc<WorkspaceService>>,
}

impl AppState {
    pub fn init(cx: &mut App) {
        // Initialize WorkspaceService with config path
        let config_path = if cfg!(debug_assertions) {
            std::path::PathBuf::from("target/workspace-config.json")
        } else {
            std::path::PathBuf::from("workspace-config.json")
        };

        // Create workspace bus
        let workspace_bus = Arc::new(std::sync::Mutex::new(
            crate::core::event_bus::workspace_bus::WorkspaceUpdateBus::new(),
        ));

        // Create workspace service and set its bus
        let mut workspace_service = WorkspaceService::new(config_path);
        workspace_service.set_workspace_bus(workspace_bus.clone());
        let workspace_service = Arc::new(workspace_service);

        let state = Self {
            invisible_panels: cx.new(|_| Vec::new()),
            agent_manager: None,
            permission_store: None,
            session_bus: SessionUpdateBusContainer::new(),
            permission_bus: PermissionBusContainer::new(),
            workspace_bus,
            welcome_session: None,
            agent_service: None,
            message_service: None,
            workspace_service: Some(workspace_service),
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Set the AgentManager after async initialization
    pub fn set_agent_manager(&mut self, manager: Arc<AgentManager>) {
        log::info!(
            "Setting AgentManager with {} agents",
            manager.list_agents().len()
        );

        // Initialize services when agent_manager is set
        let agent_service = Arc::new(AgentService::new(manager.clone()));
        let message_service = Arc::new(MessageService::new(
            self.session_bus.clone(),
            agent_service.clone(),
        ));

        self.agent_manager = Some(manager);
        self.agent_service = Some(agent_service);
        self.message_service = Some(message_service);

        log::info!("Initialized service layer (AgentService, MessageService)");
    }

    /// Set the PermissionStore
    pub fn set_permission_store(&mut self, store: Arc<PermissionStore>) {
        log::info!("Setting PermissionStore");
        self.permission_store = Some(store);
    }

    /// Get a reference to the AgentManager if initialized
    pub fn agent_manager(&self) -> Option<&Arc<AgentManager>> {
        self.agent_manager.as_ref()
    }

    /// Get the PermissionStore if set
    pub fn permission_store(&self) -> Option<&Arc<PermissionStore>> {
        self.permission_store.as_ref()
    }

    /// Set the welcome session
    pub fn set_welcome_session(&mut self, session: WelcomeSession) {
        log::info!(
            "Setting welcome session: session_id={}, agent={}",
            session.session_id,
            session.agent_name
        );
        self.welcome_session = Some(session);
    }

    /// Get the welcome session
    pub fn welcome_session(&self) -> Option<&WelcomeSession> {
        self.welcome_session.as_ref()
    }

    /// Clear the welcome session
    pub fn clear_welcome_session(&mut self) {
        log::info!("Clearing welcome session");
        self.welcome_session = None;
    }

    /// Get the AgentService
    pub fn agent_service(&self) -> Option<&Arc<AgentService>> {
        self.agent_service.as_ref()
    }

    /// Get the MessageService
    pub fn message_service(&self) -> Option<&Arc<MessageService>> {
        self.message_service.as_ref()
    }

    /// Get the WorkspaceService
    pub fn workspace_service(&self) -> Option<&Arc<WorkspaceService>> {
        self.workspace_service.as_ref()
    }
}
impl Global for AppState {}
