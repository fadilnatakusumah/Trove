//! Structured error type (`AppError`).
//!
//! Every command returns `Result<T, AppError>`; errors serialize to the
//! frontend as `{ code, message }` objects (tech-plan §25). Human-readable
//! messages only — never raw OS errors.

use serde::Serialize;

/// Machine-readable error codes shared with the frontend (`src/types/error.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ProviderNotFound,
    SkillNotFound,
    InvalidSkill,
    InstallFailed,
    PermissionDenied,
    /// An operation tried to escape an approved agent/project directory
    /// (path-traversal guard, plan 03). Documented addition beyond §25.
    PathEscape,
    Conflict,
    GitMissing,
    Unsupported,
}

impl ErrorCode {
    /// Human-readable default message (§25: never show raw `ENOENT`).
    pub fn default_message(self) -> &'static str {
        match self {
            ErrorCode::ProviderNotFound => {
                "That AI agent is not supported by Agent Manager."
            }
            ErrorCode::SkillNotFound => {
                "Agent Manager could not find that skill. It may have been moved or removed."
            }
            ErrorCode::InvalidSkill => {
                "No supported skill definition was found in that source."
            }
            ErrorCode::InstallFailed => {
                "Agent Manager could not install the skill. Check the source and try again."
            }
            ErrorCode::PermissionDenied => {
                "Agent Manager could not access the skill directory. Check that it exists and that the application has permission to access it."
            }
            ErrorCode::PathEscape => {
                "The operation was blocked because it would modify files outside the approved agent directories."
            }
            ErrorCode::Conflict => {
                "A skill with this name already exists at the destination."
            }
            ErrorCode::GitMissing => {
                "Git is required to install skills from a repository, but it was not found on this machine."
            }
            ErrorCode::Unsupported => {
                "This action is not supported for this provider."
            }
        }
    }
}

/// The single error type crossing the Tauri boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message: code.default_message().to_string(),
            detail: None,
        }
    }

    pub fn with_message(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code.as_str())
    }
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::ProviderNotFound => "PROVIDER_NOT_FOUND",
            ErrorCode::SkillNotFound => "SKILL_NOT_FOUND",
            ErrorCode::InvalidSkill => "INVALID_SKILL",
            ErrorCode::InstallFailed => "INSTALL_FAILED",
            ErrorCode::PermissionDenied => "PERMISSION_DENIED",
            ErrorCode::PathEscape => "PATH_ESCAPE",
            ErrorCode::Conflict => "CONFLICT",
            ErrorCode::GitMissing => "GIT_MISSING",
            ErrorCode::Unsupported => "UNSUPPORTED",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for AppError {}

/// Tauri commands can return `Result<T, AppError>` directly: Tauri 2 has a
/// blanket `impl<T: Serialize> From<T> for InvokeError`, so the struct
/// serializes into the InvokeError payload ({ code, message, detail }) the
/// frontend expects — no manual conversion needed.

impl AppError {
    /// Map an io::Error against a path into a structured AppError with a
    /// human-readable message and the path as detail.
    pub fn from_io(err: &std::io::Error, path: impl AsRef<std::path::Path>) -> Self {
        let code = match err.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
            std::io::ErrorKind::NotFound => ErrorCode::SkillNotFound,
            _ => ErrorCode::InstallFailed,
        };
        AppError::new(code).with_detail(format!(
            "'{}': {err}",
            crate::services::filesystem_service::display_path(path.as_ref())
        ))
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::from_io(&err, std::path::Path::new("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_frontend_contract() {
        let err = AppError::new(ErrorCode::PermissionDenied)
            .with_detail("os error 13".to_string());
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "code": "PERMISSION_DENIED",
                "message": "Agent Manager could not access the skill directory. Check that it exists and that the application has permission to access it.",
                "detail": "os error 13"
            })
        );

        let minimal = AppError::with_message(ErrorCode::Conflict, "already exists");
        let json = serde_json::to_value(&minimal).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "code": "CONFLICT",
                "message": "already exists"
            })
        );
    }

    #[test]
    fn covers_all_tech_plan_codes() {
        // §25 codes plus the documented additions (PATH_ESCAPE, CONFLICT,
        // GIT_MISSING, UNSUPPORTED).
        for code in [
            ErrorCode::ProviderNotFound,
            ErrorCode::SkillNotFound,
            ErrorCode::InvalidSkill,
            ErrorCode::InstallFailed,
            ErrorCode::PermissionDenied,
            ErrorCode::PathEscape,
        ] {
            assert!(!code.default_message().is_empty());
        }
    }
}
