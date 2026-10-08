//! Skill parsing service (plan 04).
//!
//! Shared SKILL.md frontmatter parser used by all providers whose format
//! follows the Agent Skills standard (all three do). File contents are
//! untrusted input: parsed through the size-capped reader (§23).

use std::collections::HashMap;

/// Frontmatter fields Agent Manager understands (tech-plan §9 `Skill`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkillFrontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    /// Remaining scalar key-values (e.g. license, author, compatibility).
    pub metadata: HashMap<String, String>,
}

/// Parse `---`-delimited YAML frontmatter from a SKILL.md body.
///
/// Deliberately minimal: line-oriented scalar parsing of the fields we use,
/// not a full YAML implementation. Malformed frontmatter returns `None`
/// (the skill is then treated as absent/invalid — §19 behavior), except that
/// an unparsable line is skipped rather than failing the whole block.
/// Documented ceiling: nested/list frontmatter values are ignored (Rule 12).
pub fn parse_frontmatter(content: &str) -> Option<SkillFrontmatter> {
    let mut lines = content.lines();
    // Frontmatter only counts when `---` is the very first line (the convention
    // Claude Code and Cursor both follow).
    if lines.next()?.trim() != "---" {
        return None;
    }

    let mut fm = SkillFrontmatter::default();
    for line in lines {
        let line = line.trim_end();
        if line.trim() == "---" {
            return Some(fm);
        }
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue; // list items, indented maps, junk — skipped
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().trim_matches(['"', '\'']).to_string();
        if value.is_empty() {
            continue;
        }
        match key.as_str() {
            "name" => fm.name = Some(value),
            "description" => fm.description = Some(value),
            "version" => fm.version = Some(value),
            "license" | "author" | "compatibility" | "allowed-tools" => {
                fm.metadata.insert(key, value);
            }
            _ => {}
        }
    }
    // Unterminated frontmatter block: treat as absent.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_frontmatter() {
        let fm = parse_frontmatter(
            "---\nname: react-expert\ndescription: React development assistance\nversion: \"1.0\"\nlicense: Apache-2.0\n---\n\n# Body\n",
        )
        .expect("parses");
        assert_eq!(fm.name.as_deref(), Some("react-expert"));
        assert_eq!(fm.description.as_deref(), Some("React development assistance"));
        assert_eq!(fm.version.as_deref(), Some("1.0"));
        assert_eq!(fm.metadata.get("license").map(String::as_str), Some("Apache-2.0"));
    }

    #[test]
    fn requires_frontmatter_first_line() {
        assert!(parse_frontmatter("# Just markdown\n").is_none());
        assert!(parse_frontmatter("text\n---\nname: x\n---\n").is_none());
    }

    #[test]
    fn unterminated_block_is_none() {
        assert!(parse_frontmatter("---\nname: x\n").is_none());
    }

    #[test]
    fn ignores_lists_and_unknown_keys() {
        let fm = parse_frontmatter(
            "---\nname: x\ndescription: y\npaths:\n  - \"**/*.ts\"\nunknown: z\n---\n",
        )
        .expect("parses");
        assert_eq!(fm.name.as_deref(), Some("x"));
        assert!(!fm.metadata.contains_key("paths"));
        assert!(!fm.metadata.contains_key("unknown"));
    }
}
