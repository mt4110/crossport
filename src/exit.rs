use std::process;

/// Standard Exit Codes for crossport
/// v0.5 Specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Success (0)
    Success = 0,
    /// General Error (1) - Unexpected panic or unhandled error
    GeneralError = 1,
    /// Usage Error (2) - Invalid arguments or configuration
    UsageError = 2,
    /// Scan Error (3) - System permission or tool execution failure
    ScanError = 3,
    /// Process Error (4) - Failed to kill/restart process
    ProcessError = 4,
}

impl ExitCode {
    pub fn exit(self) -> ! {
        process::exit(self as i32);
    }
}

/// Helper to handle Result -> ExitCode mapping
/// Logs error to stderr and returns appropriate ExitCode
pub fn map_err_to_code(err: &anyhow::Error) -> ExitCode {
    // We can inspect the error downcast here if we define specific error types.
    // For now, let's use a heuristic or just GeneralError/UsageError
    // To make this robust, we should probably define custom error types in core/errors.rs later.
    // For v0.5, let's assume usage errors come from clap (handled by clap mostly) or config.

    // Heuristic:
    let msg = err.to_string().to_lowercase();
    if msg.contains("usage") || msg.contains("invalid") || msg.contains("config") {
        return ExitCode::UsageError;
    }
    if msg.contains("permission") || msg.contains("access") {
        return ExitCode::ScanError;
    }
    if msg.contains("failed to kill") || msg.contains("failed to restart") {
        return ExitCode::ProcessError;
    }

    ExitCode::GeneralError
}
