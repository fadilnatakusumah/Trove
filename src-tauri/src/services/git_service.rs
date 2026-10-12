//! Git availability probe (plan 03, tech-plan §36 Step 3.5).
//!
//! Runs `git --version` with a fixed argv — never a shell string (§22).

use serde::Serialize;

use crate::errors::{AppError, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProbe {
    pub available: bool,
    /// Version string, e.g. "2.47.1", when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
}

pub fn probe_git() -> Result<GitProbe, AppError> {
    let output = std::process::Command::new("git")
        .arg("--version")
        .output()
        .map_err(|e| {
            // Spawn failure = git not on PATH. Not an internal error: the UI
            // shows an actionable "Git is required" state (plan 06).
            AppError::new(ErrorCode::GitMissing).with_detail(e.to_string())
        })?;

    if !output.status.success() {
        return Ok(GitProbe {
            available: false,
            version: None,
            error_detail: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        });
    }

    // Output looks like "git version 2.47.1".
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text
        .trim()
        .strip_prefix("git version")
        .map(str::trim)
        .map(str::to_string);

    Ok(GitProbe {
        available: true,
        version,
        error_detail: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_returns_structured_result() {
        // Environment-dependent, but the contract is: either an Ok probe
        // (available true/false) or a GitMissing AppError. Never a panic or
        // raw io::Error.
        match probe_git() {
            Ok(probe) => {
                if probe.available {
                    assert!(probe.version.is_some());
                    assert!(probe.error_detail.is_none());
                }
            }
            Err(e) => assert_eq!(e.code, ErrorCode::GitMissing),
        }
    }
}
