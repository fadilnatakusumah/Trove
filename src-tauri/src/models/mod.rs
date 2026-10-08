//! Domain models.
//!
//! `Skill`, `SkillSource`, `SkillScope`, `Project`, `ProviderId`, and
//! provider detection types. Serde-camelCase to match the TypeScript mirror
//! types in `src/types/` (plan 02).

use std::fmt;

/// Supported agent providers. Not a free string — the compiler guards
/// every match site (tech-plan §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Claude,
    Cursor,
    Codex,
    Opencode,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Cursor => "cursor",
            ProviderId::Codex => "codex",
            ProviderId::Opencode => "opencode",
        }
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a skill came from (tech-plan §9).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SkillSource {
    Local {
        path: String,
    },
    Git {
        url: String,
        /// Git ref (branch/tag/commit). `ref` is a Rust keyword, hence ref_.
        #[serde(rename = "ref")]
        ref_: Option<String>,
        /// Subdirectory inside the repository holding the skill(s). Derived
        /// from pasted file/tree links (e.g. GitHub `blob/<ref>/<path>`);
        /// `None` means the repository root.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath: Option<String>,
    },
}

/// Installation scope: global (`~/.<agent>/skills`) or project-local
/// (`<project>/.<agent>/skills`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SkillScope {
    Global,
    Project {
        #[serde(rename = "projectPath")]
        project_path: String,
    },
}

/// A discovered/installed skill (tech-plan §9). This is the exact contract
/// with the frontend `Skill` interface.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: SkillSource,
    pub provider: ProviderId,
    pub scope: SkillScope,
    pub path: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// RFC 3339 timestamp of when Agent Manager installed the skill, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_at: Option<String>,
    /// Extra frontmatter/agent-specific key-values (e.g. license, author).
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,
}

/// A registered project directory (tech-plan §15). Agent Manager only stores
/// the registration — it never copies or moves project content.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Providers detected inside this project's agent directories.
    pub provider_ids: Vec<ProviderId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract test: the serialized JSON must match the TS `Skill` interface
    /// in `src/types/skill.ts` (camelCase, tagged source/scope).
    #[test]
    fn skill_serializes_to_ts_contract() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("license".to_string(), "Apache-2.0".to_string());

        let skill = Skill {
            id: "react-expert".to_string(),
            name: "React Expert".to_string(),
            description: Some("React development assistance".to_string()),
            source: SkillSource::Git {
                url: "https://github.com/example/react-skill".to_string(),
                ref_: Some("main".to_string()),
                subpath: None,
            },
            provider: ProviderId::Claude,
            scope: SkillScope::Global,
            path: "~/.claude/skills/react-expert".to_string(),
            enabled: true,
            version: Some("1.0.0".to_string()),
            installed_at: Some("2026-10-08T00:00:00Z".to_string()),
            metadata,
        };

        let json = serde_json::to_value(&skill).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "id": "react-expert",
                "name": "React Expert",
                "description": "React development assistance",
                "source": {
                    "type": "git",
                    "url": "https://github.com/example/react-skill",
                    "ref": "main"
                },
                "provider": "claude",
                "scope": { "type": "global" },
                "path": "~/.claude/skills/react-expert",
                "enabled": true,
                "version": "1.0.0",
                "installedAt": "2026-10-08T00:00:00Z",
                "metadata": { "license": "Apache-2.0" }
            })
        );

        // Project scope + local source variant round-trips too.
        let local = Skill {
            description: None,
            version: None,
            installed_at: None,
            metadata: std::collections::HashMap::new(),
            source: SkillSource::Local {
                path: "/tmp/skill".to_string(),
            },
            scope: SkillScope::Project {
                project_path: "~/Projects/prophero".to_string(),
            },
            ..skill
        };
        let json = serde_json::to_value(&local).expect("serialize");
        assert_eq!(
            json["source"],
            serde_json::json!({ "type": "local", "path": "/tmp/skill" })
        );
        assert_eq!(
            json["scope"],
            serde_json::json!({ "type": "project", "projectPath": "~/Projects/prophero" })
        );
        assert!(json.get("description").is_none());
        assert!(json.get("metadata").is_none());

        // Deserialization accepts the same shape (round trip).
        let back: Skill = serde_json::from_value(json).expect("deserialize");
        assert_eq!(
            back.scope,
            SkillScope::Project {
                project_path: "~/Projects/prophero".to_string()
            }
        );
    }

    #[test]
    fn project_serializes_to_ts_contract() {
        let project = Project {
            id: "prophero".to_string(),
            name: "PropHero".to_string(),
            path: "~/Projects/prophero".to_string(),
            provider_ids: vec![ProviderId::Claude, ProviderId::Codex],
        };
        let json = serde_json::to_value(&project).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "id": "prophero",
                "name": "PropHero",
                "path": "~/Projects/prophero",
                "providerIds": ["claude", "codex"]
            })
        );
    }
}
