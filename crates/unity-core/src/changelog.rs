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
            | \bnow\s+(?:refuses|stops|fails)\b
            | \bno\s+longer\s+(?:registers?|installs?|deletes?|returns?|writes?|prints?|accepts?|adds?|creates?|includes?|ships?)\b
            ",
        )
        .expect("valid regex")
    })
}

/// High-signal phrases that make an exit-code change in a fix worth surfacing.
fn fix_contract_phrases() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \bnew\s+exit\s+codes?\b
            | \bfor\s+scripts\b
            | \bwithout\b.{0,80}?\benvelope\b
            | \boutput\s+shape\b
            | \bdata\.items\b
            | \bexits?\s+0\b.{0,180}?\balready\s+installed\b
            ",
        )
        .expect("valid regex")
    })
}

/// A command that now refuses an input, immediately after its `unity` name.
fn fix_command_refusal_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?isx)\bunity\s+[a-z][a-z0-9.-]*\b.{0,80}?\bnow\s+refuses?\b")
            .expect("valid regex")
    })
}

/// A command that no longer installs or deletes something, immediately after
/// its `unity` name. These are contract changes even when no exit code moves.
fn fix_command_destructive_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)\bunity\s+[a-z][a-z0-9.-]*\b.{0,80}?
            \bno\s+longer\s+(?:installs?|deletes?)\b",
        )
        .expect("valid regex")
    })
}

/// Other command behavior changes only count when the entry also mentions a
/// machine-readable contract (not a bare exit number).
fn fix_command_other_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)\bunity\s+[a-z][a-z0-9.-]*\b.{0,80}?
            \b(?:now\s+(?:stops|fails)
            | no\s+longer\s+(?:prints?|creates?|registers?|adds?))\b",
        )
        .expect("valid regex")
    })
}

fn fix_command_strong_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:\berror\s+codes?\b|--format\b|\b(?:json|ndjson|tsv)\b)")
            .expect("valid regex")
    })
}

fn fix_command_exit_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bexit(?:s|ed)?\s+(?:with\s+)?(?:code\s+)?\d+\b").expect("valid regex")
    })
}

/// An exit code paired with "instead of" wording. A passing mention of an exit
/// code alone does not qualify.
fn fix_exit_instead_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)\b(?:exits?|exited|exiting)\s+(?:with\s+)?(?:code\s+)?\d+\b
            .{0,180}?\binstead\s+of\b",
        )
        .expect("valid regex")
    })
}

/// An exit code paired with "used to" wording. This only counts when the entry
/// also changes a machine-readable format or concerns an open Editor.
fn fix_exit_used_to_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)\b(?:exits?|exited|exiting)\s+(?:with\s+)?(?:code\s+)?\d+\b
            .{0,80}?\b(?:used\s+to|where\s+it\s+used\s+to)\b",
        )
        .expect("valid regex")
    })
}

/// "It used to ... exit 0" wording. Used only when the entry also changes a
/// machine-readable format, so an old bug description alone does not match.
fn fix_reverse_exit_change_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)\b(?:used\s+to|where\s+it\s+used\s+to)\b
            .{0,100}?\b(?:exits?|exited|exiting)\s+(?:with\s+)?(?:code\s+)?\d+\b",
        )
        .expect("valid regex")
    })
}

fn fix_now_exit_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)\bnow\s+(?:exits?|exited|exiting)\b.{0,80}?\b(?:code\s+)?\d+\b")
            .expect("valid regex")
    })
}

/// The exit-code change must also touch a contract surface the app cares
/// about (machine output, an error code, or an open Editor).
fn fix_exit_contract_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:\berror\s+codes?\b|--format\b|\b(?:json|ndjson|tsv)\b|\benvelope\b|output\s+shape|\bdata\.items\b|running\s+editor|open\s+in\s+a\s+unity\s+editor|already\s+installed)",
        )
        .expect("valid regex")
    })
}

/// Output-shape changes in fixes: printed/returned data replaced by another
/// representation. Only used together with [`format_signal`].
fn fix_output_change_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \bprint(?:s|ed)?\b.{0,160}?\b(?:instead\s+of|where\s+it\s+used\s+to|used\s+to)\b
            | \bnow\s+(?:prints?|returns?|writes?|reports?)\b.{0,160}?\b(?:instead\s+of|used\s+to|where\s+it\s+used\s+to)\b
            ",
        )
        .expect("valid regex")
    })
}

fn format_signal() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:--format\b|\b(?:json|ndjson|tsv)\b|\benvelope\b|output\s+shape|\bdata\.items\b)",
        )
        .expect("valid regex")
    })
}

fn editor_signal() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:running\s+editor|open\s+in\s+a\s+unity\s+editor)").expect("valid regex")
    })
}

/// A narrower test for `Fixed`/`Security` notes. Bug-fix prose uses words such
/// as "removed" and "no longer" constantly, so only explicit contract changes
/// are surfaced here.
fn fix_contract_wording(text: &str) -> bool {
    fix_wording().is_match(text)
        || fix_contract_phrases().is_match(text)
        || (fix_command_refusal_wording().is_match(text)
            && fix_command_exit_marker().is_match(text))
        || fix_command_destructive_wording().is_match(text)
        || (fix_command_other_wording().is_match(text)
            && fix_command_strong_marker().is_match(text))
        || (fix_exit_instead_wording().is_match(text) && fix_exit_contract_marker().is_match(text))
        || (fix_exit_used_to_wording().is_match(text)
            && (format_signal().is_match(text) || editor_signal().is_match(text)))
        || (fix_reverse_exit_change_wording().is_match(text) && format_signal().is_match(text))
        || (fix_now_exit_wording().is_match(text) && fix_exit_contract_marker().is_match(text))
        || (fix_output_change_wording().is_match(text) && format_signal().is_match(text))
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

/// The CLI marks its own breaking entries with a leading `**Breaking:**`, which
/// `clean_inline` reduces to `Breaking:`. Use that marker directly instead of
/// guessing from the wording, so an explicit exit-code change is not reported
/// as a removal or rename.
fn explicitly_marked_breaking(entry: &str) -> bool {
    entry
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("breaking:")
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
            let reason = if explicitly_marked_breaking(entry) {
                Some("explicitly marked breaking")
            } else if section_lower.contains("removed") {
                Some("removed functionality")
            } else if section_lower.contains("deprecated") {
                Some("deprecated functionality")
            } else if is_fix_or_security {
                fix_contract_wording(&text_lower).then_some("behavior or output-shape change")
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
- `unity setup all` no longer registers the Unity MCP server for coding agents that can run shell commands, which use the `unity` CLI directly.
- `unity docs <topic>` answers sooner on a cold start when the topic's Editor version is known: it opens or prints the page as soon as the documentation index files read so far hold it, instead of after the whole index has loaded.

### Fixed

- `unity projects info` prints `key<TAB>value` rows under `--format tsv` and one JSON line with `"type":"result"` under `--format ndjson`, instead of the human block.
- `unity auth logout --format json` and `--format ndjson` print a success result (`{"signedOut": true}`) instead of the "You have been signed out." notice.
- `unity install-modules -m <module>` exits 0 when every module you name is already installed, as `unity editor module add` does.
- `unity projects upgrade` on a project that is already open in a Unity Editor now stops with an error that names the running Editor's process, and exits `6`. It used to launch a second Editor, which refused the open project, and report the upgrade as in progress with exit `0`.
- `unity command` exits `14` with the `EDITOR_TIMEOUT` error code when the Editor stops a command that ran past `--timeout`, instead of exiting `6` with `COMMAND_FAILED`.
- `unity uninstall` now reports the Editor it removed under `--format json`, `ndjson` and `tsv`: the json envelope and the ndjson result frame carry `version` and `architecture`, and `tsv` prints them as key and value rows. It used to print nothing and exit 0.
- `unity self-update` now exits with code 1 and the error code `UPGRADE_NO_BINARY` when the release has no download for your platform, instead of exit 6 with `COMMAND_FAILED`.
- `unity install <version> --changeset <changeset>` no longer installs the public build of that version when the changeset belongs to a different build. It now exits `11` (`RELEASE_NOT_FOUND`) and installs nothing.
- `unity uninstall` no longer deletes an Editor whose architecture the Hub's editor list does not record. Without `--architecture` it now stops with a message and removes nothing.
- `unity self-update --rollback` now refuses to run together with `--dry-run`, `--check`, `--changelog`, `--target` or `--channel`, and exits `2` without changing anything.
- `--quiet` no longer empties `--format tsv` tables. `unity --quiet --format tsv releases` printed nothing and exited 0, so a script that piped it got no data. It now prints the same rows as without `--quiet`.
- `unity collab attachments upload` no longer leaves an unfinished attachment behind when the file transfer fails, so you can run the upload again on the same annotation. If the attachment cannot be removed, the error names the `unity collab attachments delete` command that clears it.
- `unity projects --watch` now follows the project list. A project added or removed from another terminal while the watch runs appears or disappears on the next poll. Before, only the rows already shown refreshed, so the list never changed.
"#;

    const V1_0_0: &str = r#"## 1.0.0

### Changed

- **Breaking:** `unity recompile` exits `12` when no Editor answers, as `unity command` does, instead of `7`. A script that matched exit `7` for this should check `12`; `errors[0].code` is unchanged.
- `unity recompile` takes the project as a positional argument too: `unity recompile <path>`.

### Fixed

- **Breaking:** `unity close` and `unity projects close` fail with exit `11` and `PROJECTS_CLOSE_NOT_A_PROJECT` when the path isn't a Unity project (no `Assets` or `ProjectSettings` folder), such as a mistyped name. They used to succeed with `reason: "not_open"`.
- `unity install` and `unity install-modules` write their module notices, such as an added dependency or a skipped unknown module, to stderr under `--format json`, `ndjson` and `tsv`, so stdout holds only the output a script parses.
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
            "unity command exits",
            "uninstall",
            "self-update",
            "setup all",
            "changeset",
            "uninstall no longer deletes",
            "self-update --rollback",
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

        for needle in [
            "collab attachments upload",
            "projects --watch",
            "answers sooner",
            "--quiet no longer empties",
        ] {
            assert!(
                !parsed
                    .breaking_changes
                    .iter()
                    .any(|change| change.text.contains(needle)),
                "{needle} is a fix or ordinary behavior change, not a breaking change"
            );
        }
    }

    #[test]
    fn explicit_breaking_markers_get_their_own_reason() {
        let parsed = parse_changelog(V1_0_0);

        for needle in ["recompile exits", "unity close"] {
            let change = parsed
                .breaking_changes
                .iter()
                .find(|change| change.text.contains(needle))
                .unwrap_or_else(|| panic!("{needle} should be flagged"));
            assert_eq!(
                change.reason, "explicitly marked breaking",
                "{needle} should use the explicit marker reason"
            );
        }

        assert!(
            !parsed
                .breaking_changes
                .iter()
                .any(|change| change.text.contains("positional argument")),
            "an ordinary Changed entry should stay quiet"
        );
        assert!(
            !parsed
                .breaking_changes
                .iter()
                .any(|change| change.text.contains("module notices")),
            "an install fix that only moves notices to stderr is not a breaking change"
        );
    }
}
