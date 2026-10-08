//! Narrow, local-only automation transport for ClipRiva.
//!
//! The module deliberately exposes a finite protocol. It contains no generic
//! command runner, path parameter, SQL input, shell adapter, network listener,
//! MCP surface, or direct database access. The application supplies an
//! [`AutomationExecutor`] implementation so Tauri IPC and local automation can
//! eventually share the same reviewed use-case layer.

pub mod cli;
pub mod protocol;
pub mod runtime;
pub mod server;

use protocol::{
    AutomationCapability, AutomationFailure, AutomationOperation, AutomationOutput,
    AutomationStatus,
};

/// Application-owned execution boundary used by the socket server.
///
/// Authorization is intentionally checked by `server` before `execute` is
/// called. Because settings can change between admission and execution,
/// implementations must atomically re-check enabled state and the operation's
/// required capabilities before reading content or committing a side effect.
/// They must also validate repository state and must not turn operation fields
/// into arbitrary SQL, paths, commands, or endpoints.
pub trait AutomationExecutor: Send + Sync + 'static {
    fn status(&self) -> AutomationStatus;

    fn capability_enabled(&self, capability: AutomationCapability) -> bool;

    fn execute(
        &self,
        operation: &AutomationOperation,
    ) -> Result<AutomationOutput, AutomationFailure>;
}
