use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::errors::UnityError;
use crate::models::RawChangelog;
use crate::runner::UnityCli;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogSection {
    pub title: String,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BreakingChange {
    pub text: String,
    pub reason: String,
    pub section: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParsedChangelog {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub sections: Vec<ChangelogSection>,
    pub breaking_changes: Vec<BreakingChange>,
}

impl ParsedChangelog {
    pub fn entry_count(&self) -> usize {
        self.sections
            .iter()
            .map(|section| section.entries.len())
            .sum()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChangelogRequest {
    /// A Unity CLI version, or `None` for the installed CLI.
    pub target: Option<String>,
}

/// Where release notes come from.
///
/// v1 ships [`CliChangelogProvider`]. A future remote JSON provider can
/// implement this trait and be selected without changing the UI contract.
pub trait ChangelogProvider: Send + Sync {
    fn source_id(&self) -> &'static str;
    fn source_detail(&self, request: &ChangelogRequest, raw: &RawChangelog) -> String;
    fn fetch(&self, request: &ChangelogRequest) -> Result<RawChangelog, UnityError>;
}

#[derive(Clone)]
pub struct CliChangelogProvider {
    cli: UnityCli,
}

impl CliChangelogProvider {
    pub fn new(cli: UnityCli) -> Self {
        Self { cli }
    }
}

impl ChangelogProvider for CliChangelogProvider {
    fn source_id(&self) -> &'static str {
        "unity-changelog"
    }

    fn source_detail(&self, request: &ChangelogRequest, raw: &RawChangelog) -> String {
        match request.target.as_deref() {
            Some(target) => format!("Unity CLI {target} release notes"),
            None => format!("Unity CLI {}", raw.version),
        }
    }

    fn fetch(&self, request: &ChangelogRequest) -> Result<RawChangelog, UnityError> {
        self.cli.changelog(request.target.as_deref())
    }
}

/// The provider-agnostic shape a UI renders.
///
/// Raw Markdown is deliberately not stored here. It can be larger than the
/// parsed notes and is only needed when the user opens the raw view; the UI
/// fetches it on demand through [`ChangelogProvider`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogView {
    pub version: String,
    pub sections: Vec<ChangelogSection>,
    pub breaking_changes: Vec<BreakingChange>,
    pub entry_count: usize,
    pub source: String,
    pub source_detail: String,
}

impl ChangelogView {
    pub fn load(
        provider: &dyn ChangelogProvider,
        request: &ChangelogRequest,
    ) -> Result<Self, UnityError> {
        let raw = provider.fetch(request)?;
        let parsed = parse_changelog(&raw.changelog);
        let version = parsed
            .version
            .clone()
            .unwrap_or_else(|| raw.version.clone());
        let entry_count = parsed.entry_count();
        let source_detail = provider.source_detail(request, &raw);
        Ok(Self {
            version,
            sections: parsed.sections,
            breaking_changes: parsed.breaking_changes,
            entry_count,
            source: provider.source_id().to_string(),
            source_detail,
        })
    }
}

fn breaking_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(breaking|incompatible|removal|removed|renamed|replaced|deprecated|must now|drop(?:ped|s)? support|no longer (?:supported|available|accepted|valid|works?))\b",
        )
        .expect("valid regex")
    })
}

/// Fix and security notes often say a bug "no longer" happens. That is not a
/// breaking change, so those sections only flag explicit breaking or
/// incompatible wording, plus the high-signal behavior changes below.
fn fix_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(breaking|incompatible)\b").expect("valid regex"))
}

/// Wording that describes a contract change rather than a bug fix: changed
/// exit codes, output shapes, or a command that now refuses an input.
fn behavior_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \bnow\s+(?:exits?|returns?|prints?|writes?|reports?|requires?|refuses?|stops?|fails?|accepts?|installs?|registers?|uses?|builds?)\b
            .{0,160}?\b(?:instead\s+of|where\s+it\s+used\s+to|used\s+to)\b
            | \bused\s+to\b
            | \bexit(?:s|ed)?\s+(?:code\s+)?\d+\b
            | \bexit\s+codes?\b
            | \bfor\s+scripts\b
            | \boutput\s+shape\b
            | \benvelope\b
            | \bdata\.items\b
            | \b(?:print|prints|printed|write|writes|written|return|returns|returned|report|reports|reported|emit|emits|emitted|output|outputs|outputted)\b
              .{0,120}?\b(?:tsv|ndjson)\b
            | \bprint(?:s|ed)?\b.{0,120}?\binstead\s+of\b
            | \bnow\s+(?:refuses|stops|fails)\b
            | \bno\s+longer\s+(?:registers?|installs?|deletes?|returns?|writes?|prints?|accepts?|adds?|creates?|includes?|ships?)\b
            ",
        )
        .expect("valid regex")
    })
}

fn markdown_link() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").expect("valid regex"))
}

/// Parses the Markdown returned by `unity changelog --format json`.
///
/// The CLI currently returns one release's notes, but the parser tolerates
/// multiple `##` version headings so it keeps working if that changes.
pub fn parse_changelog(markdown: &str) -> ParsedChangelog {
    let mut parsed = ParsedChangelog::default();
    let mut current_section: Option<usize> = None;
    let mut current_entry: Option<usize> = None;
    let mut in_fence = false;

    for raw_line in markdown.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            if let (Some(section), Some(entry)) = (current_section, current_entry) {
                if let Some(text) = parsed.sections[section].entries.get_mut(entry) {
                    if !trimmed.is_empty() {
                        text.push(' ');
                        text.push_str(trimmed);
                    }
                }
            }
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("### ") {
            let title = clean_inline(heading);
            parsed.sections.push(ChangelogSection {
                title,
                entries: Vec::new(),
            });
            current_section = Some(parsed.sections.len() - 1);
            current_entry = None;
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("## ") {
            let value = clean_inline(heading);
            if value.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                && parsed.version.is_none()
            {
                parsed.version = Some(value);
            }
            current_section = None;
            current_entry = None;
            continue;
        }

        // Sub-headings inside a section are retained as their own entries.
        if let Some(heading) = trimmed.strip_prefix("#### ") {
            let text = clean_inline(heading);
            push_entry(&mut parsed, &mut current_section, &mut current_entry, text);
            continue;
        }

        let bullet = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "));
        if let Some(text) = bullet {
            push_entry(
                &mut parsed,
                &mut current_section,
                &mut current_entry,
                text.to_string(),
            );
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Continuation of a wrapped bullet.
        if let (Some(section), Some(entry)) = (current_section, current_entry) {
            if let Some(text) = parsed.sections[section].entries.get_mut(entry) {
                text.push(' ');
                text.push_str(&clean_inline(trimmed));
            }
        }
    }

    parsed.breaking_changes = find_breaking_changes(&parsed.sections);
    parsed
}

fn push_entry(
    parsed: &mut ParsedChangelog,
    current_section: &mut Option<usize>,
    current_entry: &mut Option<usize>,
    raw: String,
) {
    let section = match *current_section {
        Some(index) => index,
        None => {
            parsed.sections.push(ChangelogSection {
                title: "Release notes".to_string(),
                entries: Vec::new(),
            });
            *current_section = Some(parsed.sections.len() - 1);
            parsed.sections.len() - 1
        }
    };
    let text = clean_inline(raw.trim());
    if text.is_empty() {
        return;
    }
    parsed.sections[section].entries.push(text);
    *current_entry = Some(parsed.sections[section].entries.len() - 1);
}

fn find_breaking_changes(sections: &[ChangelogSection]) -> Vec<BreakingChange> {
    let mut seen = HashSet::new();
    let mut changes = Vec::new();

    for section in sections {
        let section_lower = section.title.to_ascii_lowercase();
        let is_fix_or_security =
            section_lower.contains("fixed") || section_lower.contains("security");
        for entry in &section.entries {
            let text_lower = entry.to_ascii_lowercase();
            let reason = if section_lower.contains("removed") {
                Some("removed functionality")
            } else if section_lower.contains("deprecated") {
                Some("deprecated functionality")
            } else if is_fix_or_security {
                if fix_wording().is_match(&text_lower) {
                    Some("explicit breaking or incompatible wording in a fix")
                } else if behavior_wording().is_match(&text_lower) {
                    Some("behavior or output-shape change")
                } else {
                    None
                }
            } else if breaking_wording().is_match(&text_lower) {
                // Do not blanket-flag every Changed entry: a release's Changed
                // section is mostly ordinary behavior updates. Only wording
                // that signals removal, a rename, or an incompatibility is a
                // useful "possible breaking change" candidate.
                Some("removal, rename, or deprecation wording")
            } else if behavior_wording().is_match(&text_lower) {
                Some("behavior or output-shape change")
            } else {
                None
            };

            let Some(reason) = reason else {
                continue;
            };

            let key = text_lower.trim().to_string();
            if !seen.insert(key) {
                continue;
            }
            changes.push(BreakingChange {
                text: entry.clone(),
                reason: reason.to_string(),
                section: section.title.clone(),
            });
        }
    }

    changes
}

fn clean_inline(value: &str) -> String {
    let value = markdown_link().replace_all(value, "$1 ($2)");
    value
        .replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"## 1.0.0-beta.14

### Added

- `unity config path` prints where the CLI keeps its configuration.
- A new flag that wraps onto
  a second line.

### Changed

- `unity status` now reports Safe Mode.
- The old `projects --add` flag is deprecated.

### Removed

- `unity legacy-run` was removed.

### Security

- `unity doctor` redacts arguments in its recent log.

### Fixed

- A crash when the network was unreachable.
"#;

    const BETA16: &str = r#"## 1.0.0-beta.16

### Changed

- Two new exit codes: `17` (`UNAVAILABLE`) when the Editor is busy loading, compiling or in Safe Mode, and `18` (`AWAITING_INPUT`) when a modal dialog blocks it.
- A command that cannot reach its service because you are offline now exits `7` (`NETWORK_UNREACHABLE`) instead of `6`.
- For scripts: against an Editor that formats its output, `unity command --format json` prints the result without the `success`, `command`, `parameters` and `target` envelope.
- `unity docs cache --format json` now returns the cached versions in `data.items`, next to `total`, `nextPage` and `partial`.
- `unity projects info` prints `key<TAB>value` rows under `--format tsv` and one JSON line with `"type":"result"` under `--format ndjson`, instead of the human block.
- `unity auth logout --format json` and `--format ndjson` print a success result (`{"signedOut": true}`) instead of the "You have been signed out." notice.

### Fixed

- `unity install-modules -m <module>` exits 0 when every module you name is already installed, as `unity editor module add` does.
- `unity projects upgrade` on a project that is already open in a Unity Editor now stops with an error that names the running Editor's process, and exits `6`. It used to launch a second Editor, which refused the open project, and report the upgrade as in progress with exit `0`.
- `unity collab attachments upload` no longer leaves an unfinished attachment behind when the file transfer fails, so you can run the upload again on the same annotation. If the attachment cannot be removed, the error names the `unity collab attachments delete` command that clears it.
- `unity projects --watch` now follows the project list. A project added or removed from another terminal while the watch runs appears or disappears on the next poll. Before, only the rows already shown refreshed, so the list never changed.
"#;

    #[test]
    fn parses_versions_sections_and_wrapped_entries() {
        let parsed = parse_changelog(SAMPLE);
        assert_eq!(parsed.version.as_deref(), Some("1.0.0-beta.14"));
        assert_eq!(parsed.sections.len(), 5);
        let added = &parsed.sections[0];
        assert_eq!(added.title, "Added");
        assert_eq!(added.entries.len(), 2);
        assert!(added.entries[1].contains("second line"));
        assert!(added.entries[0].contains("unity config path"));
    }

    #[test]
    fn flags_removed_and_deprecated_entries() {
        let parsed = parse_changelog(SAMPLE);
        let texts: Vec<&str> = parsed
            .breaking_changes
            .iter()
            .map(|change| change.text.as_str())
            .collect();
        assert!(texts.iter().any(|text| text.contains("deprecated")));
        assert!(texts.iter().any(|text| text.contains("legacy-run")));
        assert!(
            !texts.iter().any(|text| text.contains("Safe Mode")),
            "ordinary Changed entries should not be blanket-flagged"
        );
        assert!(
            !texts.iter().any(|text| text.contains("redacts arguments")),
            "security fixes should not be labelled breaking"
        );
    }

    #[test]
    fn flags_explicit_breaking_wording_outside_changed_sections() {
        let parsed = parse_changelog(
            "### Added\n\n- The token format is incompatible with older clients.\n",
        );
        assert_eq!(parsed.breaking_changes.len(), 1);
        assert_eq!(
            parsed.breaking_changes[0].reason,
            "removal, rename, or deprecation wording"
        );
    }

    #[test]
    fn does_not_flag_security_entries_just_for_being_in_security() {
        let parsed = parse_changelog("### Security\n\n- Something was fixed.\n");
        assert!(parsed.breaking_changes.is_empty());
    }

    #[test]
    fn beta16_behavior_and_output_changes_are_flagged() {
        let parsed = parse_changelog(BETA16);

        for needle in [
            "Two new exit codes",
            "now exits",
            "For scripts",
            "data.items",
            "projects info",
            "auth logout",
            "install-modules",
            "projects upgrade",
        ] {
            let change = parsed
                .breaking_changes
                .iter()
                .find(|change| change.text.contains(needle))
                .unwrap_or_else(|| panic!("{needle} should be flagged"));
            assert_eq!(
                change.reason, "behavior or output-shape change",
                "{needle} should use the behavior reason"
            );
        }
    }

    #[test]
    fn beta16_fix_wording_false_positives_stay_quiet() {
        let parsed = parse_changelog(BETA16);

        for needle in ["collab attachments upload", "projects --watch"] {
            assert!(
                !parsed
                    .breaking_changes
                    .iter()
                    .any(|change| change.text.contains(needle)),
                "{needle} is a fix, not a breaking change"
            );
        }
    }
}
