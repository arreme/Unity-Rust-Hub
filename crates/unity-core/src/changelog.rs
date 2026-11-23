use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::errors::UnityError;
use crate::models::RawChangelog;
use crate::runner::UnityCli;
use crate::surface;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogSection {
    pub title: String,
    pub entries: Vec<ChangelogEntry>,
}

/// The documentation collection an entry's best candidate belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DocsTopicKind {
    /// A class-like identifier such as `MonoBehaviour.Start`.
    #[default]
    ScriptReference,
    /// A CLI command path such as `install-modules` or `pipeline editors`.
    Cli,
}

/// One release-note entry: the readable text plus what the app can do with it.
///
/// `app_impact` is true when the entry mentions a command the adapter runs, a
/// flag it passes, or an error/exit code it maps. `docs_topic` is the preferred
/// backticked candidate: a command sequence wins over a class-like identifier,
/// and flags/placeholders are never topics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    pub text: String,
    pub app_impact: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_topic: Option<String>,
    pub docs_kind: DocsTopicKind,
    /// The raw Markdown, retained only so wrapped and fenced continuations can
    /// be reclassified; it is not part of the view contract.
    #[serde(skip)]
    raw: String,
}

impl ChangelogEntry {
    /// Builds one entry from raw Markdown, as the parser does. Public so the
    /// controller can build deterministic fixtures without reimplementing the
    /// classifier.
    pub fn new(raw: &str) -> Self {
        let mut entry = Self::default();
        entry.append(raw);
        entry
    }

    /// Appends one wrapped bullet or fenced line and recomputes the signals
    /// over the whole entry.
    fn append(&mut self, raw: &str) {
        let cleaned = clean_inline(raw);
        if !cleaned.is_empty() {
            if !self.text.is_empty() {
                self.text.push(' ');
            }
            self.text.push_str(&cleaned);
        }
        if !self.raw.is_empty() {
            self.raw.push(' ');
        }
        self.raw.push_str(raw);
        self.reclassify();
    }

    fn reclassify(&mut self) {
        let signals = classify_entry(&self.raw);
        self.app_impact = signals.app_impact;
        self.docs_topic = signals.docs_topic;
        self.docs_kind = signals.docs_kind;
    }
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

/// Script-facing output changes: a piped table gained a header row, a bare
/// value became a table, or the notes now tell scripts to pass
/// `--format human`. These break pipelines without renaming a command.
fn script_output_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \bnow\s+starts?\s+with\b.{0,140}?\b(?:header\s+row|key\s+and\s+value)\b
            | \bnow\s+captures?\s+a\s+table\b
            | \bcaptures?\s+a\s+table\b
            | \bbare\b.{0,120}?\b--format\s+human\b
            | \b--format\s+human\b.{0,120}?\bbare\b
            | \bnow\s+(?:prints?|returns?|writes?)\b.{0,160}?\bnot\s+the\s+bare\b
            ",
        )
        .expect("valid regex")
    })
}

/// Option values that used to be coerced are now refused. Callers that pass
/// `7rows`, `1e3` or `0x50` where a whole number is expected break.
fn strict_value_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \bplain\s+whole\s+numbers?\b
            | \b(?:are|is)\s+now\s+usage\s+errors?\b
            | \bnow\s+usage\s+errors?\b
            ",
        )
        .expect("valid regex")
    })
}

/// A search or filter moved to a different expression language, so existing
/// patterns are refused instead of being reinterpreted.
fn syntax_change_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \bregular\s+expression\s+syntax\b.{0,180}?\b(?:refus(?:ed|es)|lookahead|lookbehind|backreferences?)\b
            | \b(?:lookahead|lookbehind|backreferences?)\b.{0,180}?\brefus(?:ed|es)\b
            ",
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

/// "The message and the exit code stay the same" is the opposite of a contract
/// change, so such an entry must not be flagged just for mentioning an exit
/// code.
fn unchanged_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \b(?:exit\s+codes?|error\s+codes?|messages?|output)\b
            .{0,80}?
            \b(?:stays?|remains?)\s+the\s+same\b
            | \b(?:exit\s+codes?|error\s+codes?|messages?)\b
            .{0,80}?
            \bunchanged\b
            | \bno\s+change\s+to\s+(?:the\s+)?(?:exit\s+codes?|messages?|output)\b
            ",
        )
        .expect("valid regex")
    })
}

/// A `Fixed` entry that moves a command's exit code: "now fails with exit code
/// 6 ... It used to exit 0".
fn fix_now_exit_change_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)
            \bnow\s+(?:exits?|fails?|stops?|prints?|returns?)\b
            .{0,200}?
            (?:exit\s+code\s+\d+|exits?\s+(?:with\s+)?\d+)
            .{0,300}?
            \bused\s+to\s+(?:exit|print|return)\b
            ",
        )
        .expect("valid regex")
    })
}

/// A `Fixed` entry that changes how a provider failure is described, naming the
/// new stable error code: "no longer say a token is invalid ... exit 7 with
/// `VCS_PROVIDER_UNREACHABLE`".
fn fix_diagnosis_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?isx)\bno\s+longer\s+(?:say|says|report|reports|treat|treats|describe|describes|blame|blames)\b",
        )
        .expect("valid regex")
    })
}

fn named_error_code() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // The heuristic lowercases entries before matching, so a code such as
        // `VCS_PROVIDER_UNREACHABLE` arrives here in lowercase.
        Regex::new(r"(?i)\b[a-z][a-z0-9]+(?:_[a-z0-9]+){2,}\b").expect("valid regex")
    })
}

/// A `Fixed` entry that changes what a named flag does and what a run exits
/// with: "`install-modules` now respects `--architecture` ... exits 2 ...
/// instead of using the installed build".
fn fix_flag_contract_wording() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?isx)\B--[a-z][a-z0-9-]+\b.{0,400}?\binstead\s+of\b").expect("valid regex")
    })
}

/// A narrower test for `Fixed`/`Security` notes. Bug-fix prose uses words such
/// as "removed" and "no longer" constantly, so only explicit contract changes
/// are surfaced here.
fn fix_contract_wording(text: &str) -> bool {
    fix_wording().is_match(text)
        || fix_contract_phrases().is_match(text)
        || fix_now_exit_change_wording().is_match(text)
        || (fix_diagnosis_wording().is_match(text)
            && fix_command_exit_marker().is_match(text)
            && named_error_code().is_match(text))
        || (fix_flag_contract_wording().is_match(text) && fix_command_exit_marker().is_match(text))
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
        || script_output_wording().is_match(text)
        || strict_value_wording().is_match(text)
        || syntax_change_wording().is_match(text)
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
                if let Some(entry) = parsed.sections[section].entries.get_mut(entry) {
                    if !trimmed.is_empty() {
                        entry.append(trimmed);
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
            if let Some(entry) = parsed.sections[section].entries.get_mut(entry) {
                entry.append(trimmed);
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
    let entry = ChangelogEntry::new(raw.trim());
    if entry.text.is_empty() {
        return;
    }
    parsed.sections[section].entries.push(entry);
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
            let text_lower = entry.text.to_ascii_lowercase();
            // "The exit code stays the same" is not a contract change, even
            // though it names an exit code. Explicit breaking wording still
            // counts on its own.
            let unchanged =
                unchanged_wording().is_match(&text_lower) && !fix_wording().is_match(&text_lower);
            let reason = if explicitly_marked_breaking(&entry.text) {
                Some("explicitly marked breaking")
            } else if section_lower.contains("removed") {
                Some("removed functionality")
            } else if section_lower.contains("deprecated") {
                Some("deprecated functionality")
            } else if is_fix_or_security {
                (!unchanged && fix_contract_wording(&text_lower))
                    .then_some("behavior or output-shape change")
            } else if unchanged {
                None
            } else if script_output_wording().is_match(&text_lower)
                || strict_value_wording().is_match(&text_lower)
                || syntax_change_wording().is_match(&text_lower)
            {
                // Report the output shape before a stray "removed" elsewhere in
                // the same entry turns a silent script break into a removal.
                Some("behavior or output-shape change")
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
                text: entry.text.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct EntrySignals {
    app_impact: bool,
    docs_topic: Option<String>,
    docs_kind: DocsTopicKind,
}

fn backtick_span() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`([^`\n]+)`").expect("valid regex"))
}

fn prose_command() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bunity\s+([a-z][a-z0-9-]*)\b").expect("valid regex"))
}

fn flag_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?:^|[^A-Za-z0-9_-])(-{1,2}[A-Za-z][A-Za-z0-9-]*)"#).expect("valid regex")
    })
}

fn error_code_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[A-Z][A-Z0-9_]{2,}\b").expect("valid regex"))
}

fn exit_code_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\bexit(?:s|ed)?\s+(?:with\s+)?(?:code\s+)?`?(\d+)`?").expect("valid regex")
    })
}

/// Classifies one entry from its raw Markdown.
///
/// Backticked spans provide the docs candidates. A `unity <command>
/// [<subcommand>]` sequence wins over a class-like identifier; flags and
/// argument placeholders are stripped. Candidate commands and flags are
/// matched against [`crate::surface`], so the filter vocabulary is the
/// adapter's, not a hand-copy in the UI.
fn classify_entry(raw: &str) -> EntrySignals {
    let mut signals = EntrySignals::default();
    let mut cli_topic = None;
    let mut class_topic = None;

    for capture in backtick_span().captures_iter(raw) {
        let Some(span) = capture.get(1) else {
            continue;
        };
        let tokens = span_tokens(span.as_str());
        if cli_topic.is_none() {
            if let Some(path) = surface::command_path(&tokens) {
                cli_topic = Some(path);
            }
        }
        if class_topic.is_none() {
            for token in &tokens {
                if let Some(identifier) = class_identifier(token) {
                    class_topic = Some(identifier);
                    break;
                }
            }
        }
    }

    if let Some(topic) = cli_topic {
        signals.docs_topic = Some(topic);
        signals.docs_kind = DocsTopicKind::Cli;
    } else if let Some(topic) = class_topic {
        signals.docs_topic = Some(topic);
        signals.docs_kind = DocsTopicKind::ScriptReference;
    }

    let backticked_command = signals.docs_kind == DocsTopicKind::Cli;
    signals.app_impact = backticked_command
        || mentions_used_command(raw)
        || mentions_used_flag(raw)
        || mentions_mapped_error(raw);
    signals
}

fn span_tokens(span: &str) -> Vec<&str> {
    span.split_whitespace()
        .map(trim_token)
        .filter(|token| !token.is_empty())
        .collect()
}

fn trim_token(token: &str) -> &str {
    token.trim_matches(|character: char| {
        matches!(
            character,
            ',' | ';' | ':' | '(' | ')' | '[' | ']' | '"' | '\'' | '!' | '?'
        )
    })
}

fn class_identifier(token: &str) -> Option<String> {
    let token = trim_token(token);
    if token.starts_with('-') || token.contains('/') {
        return None;
    }
    let parts = token.split('.').collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }
    for part in &parts {
        let mut characters = part.chars();
        let first = characters.next()?;
        if !first.is_ascii_uppercase()
            || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return None;
        }
    }
    Some(token.to_string())
}

fn mentions_used_command(raw: &str) -> bool {
    prose_command().captures_iter(raw).any(|capture| {
        capture
            .get(1)
            .is_some_and(|command| surface::is_command(command.as_str()))
    })
}

fn mentions_used_flag(raw: &str) -> bool {
    flag_token().captures_iter(raw).any(|capture| {
        capture
            .get(1)
            .is_some_and(|flag| surface::is_flag(flag.as_str()))
    })
}

fn mentions_mapped_error(raw: &str) -> bool {
    if error_code_token()
        .find_iter(raw)
        .any(|code| surface::is_error_code(code.as_str()))
    {
        return true;
    }
    exit_code_token().captures_iter(raw).any(|capture| {
        capture
            .get(1)
            .and_then(|code| code.as_str().parse::<i32>().ok())
            .is_some_and(surface::is_mapped_exit_code)
    })
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

    const V1_0_1: &str = r#"## 1.0.1

### Changed

- `unity analytics status`, `unity skill install`, the summary lines of `unity projects report` and the signed-out message of `unity cloud` now print their intro, tip and refusal lines on stderr instead of stdout, so `--quiet` no longer changes what stdout holds. Under `--format github` the signed-out `unity cloud` message is now an error annotation on stdout.
- `unity commands --format json` and `--format ndjson` now give every command a `path` key, the full command path such as `analytics opt-in`, so the `path` column of `--format tsv` names a key in the JSON output.
- `--format tsv` (the default when output is piped) now starts with a `key` and `value` header row, then one row per field, for `unity version`, `unity --version --format tsv`, `unity config get`, `set`, `unset`, `path`, `resolve` and `update-check`, `unity cache key`, `unity language`, and `unity upgrade` with `--check` or `--dry-run`. The field names are the ones `--json` uses. `--no-header` leaves the header out. To capture the bare cache key in a script, run `unity cache key --format human`; a GitHub Actions workflow made by an earlier `unity ci init` fails at its cache key step until you add that flag or regenerate it with `unity ci init --overwrite`.
- `unity upgrade --format ndjson` with `--check` or `--dry-run`, or when the CLI is already up to date, prints one result frame with the same fields as `--json`, where it used to print a tab-separated row.
- `--format tsv` (the default when output is piped) now starts with a header row for `unity editors`, `unity cloud org current`, `unity auth token`, `unity license`, `unity doctor --ci`, `unity docs --url`, `unity assets export` and the `unity command` dialog answer; `--no-header` leaves it out. A piped `$(unity auth token)`, `unity editors path` or `unity docs --url` now captures a table, so scripts pass `--format human` for the bare value. `unity doctor --ci` tsv now prints its `--json` fields as `key` and `value` rows (`verdict`, then `checks.0.id`, `checks.0.status` and so on), where it printed `verdict`, `check.<id>` and `hint.<id>` rows.
- `unity editors path --format ndjson` now prints the same data as `--json` as one result line, not the bare path.
- `--format tsv` (the default when output is piped) now starts with a header row for `unity setup` (every client and `setup all`), `unity skill install --all` and `unity self-install --print-target`. `setup` and `self-install --print-target` print `key` and `value` rows, one per field; `skill install --all` prints one row per skill record under the `--json` keys. `--no-header` leaves the header out. To capture the bare `--print-target` path in a script, run it with `--format human`.
- **Breaking:** `unity completion <shell>` with `--json`, `--format json` or `--format ndjson` writes the standard JSON envelope, with the shell name in `data.shell` and the script in `data.script`, instead of the raw script. To save a script, run `unity completion <shell>` without a machine format. The command keeps its documented JSON contract.
- `unity commands --grep` patterns use Rust regular expression syntax, so lookahead, lookbehind and backreferences are refused with a usage error, and a literal brace needs escaping (`\{json\}`). With `--shard` and `--rerun-failed`, a `--filter` entry that is not a valid Rust expression is refused with the reason instead of matching no tests. A plain `unity test --filter` is still passed to the Editor unchanged.
- `unity test`, `unity test --affected` and `unity build` now sort names by code point. Only names that mix an emoji with a full-width or other character at U+E000 or above can notice. A `last-build.json` that escapes a lone surrogate now counts as no previous build.
- Numeric options such as `--port`, `--pid`, `--lines`, `--tail` and `--limit` take plain whole numbers. Values like `7rows`, `1e3` and `0x50` are now usage errors instead of being read as a number.
- `unity ci init` no longer refuses a project whose root is spelled with a trailing slash when a symlink sits above that root.
- A folder whose `ProjectSettings/ProjectVersion.txt` is a link to nothing, a pipe or a device is no longer treated as a Unity project. Only a regular file, or a link to one, counts.

### Fixed

- `unity builds log` and `unity jobs logs` now print a `line` header row under `--format tsv`, then one row per log line, as every other command does. `unity jobs logs` no longer prints its per-step banner lines under tsv, so every row is a log line. `--no-header` leaves the header out, and an empty log is the header alone. Human, GitHub, json and ndjson output is unchanged.
- `unity install modules` installs modules for a Unity version that is promoted but not yet published, on Windows. The module list now comes from the release data for that version instead of being refused as unverifiable.
"#;

    const BETA17: &str = r##"### Added

- `unity plugin install` and `unity plugin upgrade` warn you before they download a beta build of a plugin, so you know it isn't a stable release. With `--format json` the warning is in the output's `warnings`.

- A plugin can declare other plugins it needs. `unity plugin install` and `unity plugin upgrade` install the required ones first, each with its own license step, `unity plugin remove` refuses a plugin an installed plugin requires, and `unity plugin --json` lists each entry's `dependencies`.

- `unity install --headless` installs the headless Linux Editor, which runs with no display. The version picker also asks which one you want, with the regular Editor selected. Without either, you get the regular Editor, even on a machine with no display. The headless Editor leaves out the modules it can't use, such as iOS.

- `unity pipeline automation apps versions create <app-id> --version <version>` adds a draft version to a Pipeline Automation app, empty or copied from an existing version with `--base-version`.

- `unity pipeline automation apps create` creates a Pipeline Automation app and its first draft version from options, or from a JSON file with `--file`.

- `unity pipeline cloud-build builds timeline <build-number> --build-target <target>` shows how many seconds each phase of a build took, from waiting for a machine through upload. It is available on staging only for now, and on production it stops with an error before sending any request.

### Changed

- `unity skill install` installs the Unity skills from a newer release of the Unity-Technologies/skills repository. The `unity-cli` skill now covers streamed command output and the `antigravity` and `qwen` skills clients, and `new-unity-project` now upgrades a project to Unity 6000.7 or later with `unity projects upgrade`.

- `unity docs` opens the common page for a bare name that several script reference pages share: `unity docs Start` opens `MonoBehaviour.Start` and `unity docs Log` opens `Debug.Log`, instead of a search. A name with no clear common owner still opens a search.

- `unity plugin upgrade` updates a plugin's agent skill only where it is already installed: user-wide, or in the current project. Run it from a project to refresh that project's copy. It no longer copies the skill into every AI client that has Unity's skill.

- `unity plugin install <id> --local` writes the plugin's agent skill into the current project, for each AI client whose project already has Unity's skill. Claude Desktop is skipped, because its skills are global only.

- `unity projects link vcs --rollback --delete-remote` no longer deletes the remote repository unconfirmed when it can't ask (a machine format, `--non-interactive`, no terminal on stdin, or CI). It now fails with `LINK_VCS_DELETE_REMOTE_REQUIRES_YES` (exit `2`) before it reads or detaches anything, only where a delete could happen (a run with no origin or a self-hosted origin is unchanged); pass the new `--yes` (`-y`) to delete without asking. On a terminal it still asks, and `--yes` skips the question.

- A `unity self-update` that fails because of your machine or network no longer sends a crash report: a download that stalls, a download that doesn't match its checksum or goes missing, an operating system that refuses to start the new binary, a denied or full disk, and a file the system won't replace. The message and the exit code stay the same, and the error no longer suggests `unity bug` for the checksum, stall and blocked-launch cases.

- When the Unity release service can't be reached, `unity install` and `unity editors default latest` or `lts` now fall back to Unity 6000.0.85f2 and 6000.3.28f1 LTS, up from 6000.0.64f1 and 6000.3.10f1.

- Crash reports leave out problems on your side, such as a full disk, a denied permission, a network failure or not being signed in, and the same failure is reported at most once an hour. A crash no longer holds up the prompt for more than about a second and a half when you run the CLI in a terminal.

- If you opted in to analytics, a crash report carries the same anonymous device id analytics uses. Without that consent it carries no id.

- Turning off **Send crash reports** in the Unity Hub's settings now turns off the CLI's crash reports too. `UNITY_NO_CRASH_REPORT` still turns them off on its own.

- Updated usage analytics defaults for new installs.

### Fixed

- `unity mcp` waits for an asynchronous Editor test run to finish, and marks a capture that writes a file as able to overwrite one, with Pipeline packages that name their parameters in kebab-case (`async-tests`, `output-path`).

- The install scripts install the latest beta again when no stable release exists yet, instead of failing. `UNITY_CLI_CHANNEL` set to a channel still installs only from that channel.

- `unity pipeline list`, `unity status` and every command that finds a running Editor by its project now find an Editor launched with its `-projectPath` argument in quotes, such as `"-projectPath" "C:\My Project"`. Such an Editor was not listed at all.

- `unity skill install`, `unity skill refresh` and `unity setup` install the latest published Unity skills, even on a machine that has never run the CLI before. Previously a first install could write the skills from before the most recent update. When the skills registry can't be reached, only the skills built into the CLI are installed, and the `unity-cli` skill always matches your CLI version.

- `unity skill refresh --format json` and `--format ndjson` now write a result for every run. The JSON envelope or the final `result` line lists each tracked install the refresh looked at, with what it did: `refreshed`, `up-to-date`, `pruned`, `skipped` or `conflict` (`would-update`, `would-skip` or `would-prune` under `--dry-run`), plus a `skipReason` when the row was left alone or pruned for a reason. Before, they printed nothing, even when there was nothing to refresh.

- An extra argument on `unity cloud`, `unity context`, `unity build`, `unity pipeline` and `unity projects` commands (except `unity pipeline automation` and `unity pipeline cloud-build`), for example `unity cloud status extra`, now prints `error: too many arguments` and exits `2`, like every other command. It used to exit `1` with "isn't supported with these options". A missing required argument on these commands (again except `unity pipeline automation` and `unity pipeline cloud-build`), for example `unity context use`, now prints `error: missing required argument` and exits `2` instead of `1`.

- `unity vcs setup`, `unity projects create`, `unity projects link vcs` and `unity projects clone --vcs` no longer say a GitHub or GitLab token is invalid when the provider can't be reached. They exit 7 with `VCS_PROVIDER_UNREACHABLE`, and still exit 3 with `VCS_TOKEN_INVALID` when the provider rejects the token.

- `unity ci init --provider gitlab` now generates a job that runs in a project folder whose name starts with a dash, such as `-rf`.

- `unity pipeline install` and `unity pipeline upgrade` under `--format tsv` now print their result as `key<TAB>value` lines under a header row, one line per field (`packageId`, `version`, `projectPath`, `alreadyInstalled`; and `previousVersion`, `upgraded`, `alreadyLatest` for upgrade). Before, a script got one headerless row of positional cells and could not tell which cell was which. The `json` and `ndjson` output is unchanged.

- `unity install-modules` now respects `--architecture` when one build of the version is installed: an invalid value exits 2 in every mode, and an architecture that is not installed exits 6 with a not-found message for `--list`, `--all`, named modules (`-m`) and the module picker, instead of using the installed build.

- `unity projects create`, `unity projects link vcs` (flag form and URL form) and `unity vcs setup` with `--git-token-from-stdin` (or the deprecated `--git-token-stdin`) no longer fall back to a stored login (a Git credential helper, `gh` or `glab`) when stdin is empty. They now fail with exit code 4 (`VCS_TOKEN_REQUIRED`) and create or push nothing, so a script that pipes an empty variable cannot act as whoever is signed in. `UNITY_GITHUB_TOKEN` and `UNITY_GITLAB_TOKEN` still apply. An interactive terminal on stdin reads no token, so the flag in a terminal now fails the same way instead of using the stored login.

- `unity projects create --cloud-project <name>` now fails with exit code 6 when the organization has no project, or more than one, matching that name. It used to exit 0 with only a warning. The project is still created locally, unlinked, and the error names its path.

- Creating a Unity Cloud project with `--coppa unspecified` now works on `unity projects create`, `projects link cloud`, `projects link vcs` and `vcs setup`. The Cloud request was refused with HTTP 400, so the project stayed unlinked.

- `unity projects create --vcs github` or `--vcs gitlab` with a `--git-token` now exits `7` (`VCS_PROVIDER_UNREACHABLE`) when the token check cannot reach the provider (offline, a failed DNS lookup, a refused connection, or a proxy that is down), and `14` (`ETIMEDOUT` or `ECONNABORTED`) when it times out, instead of reporting the token as invalid (exit `3`). Nothing is written in either case.

- `--quiet` no longer blanks the human answer of commands whose result is a sentence or a table. `unity docs cache --quiet` now prints "No documentation is cached..." on stdout instead of nothing, and `context`, `auth accounts`, `config`, `editors running`, `skill`, `doctor` and the other commands listed in the deviation keep their answer under `--quiet`. The `plugin` not-installed hint now goes to stderr, so `--quiet` drops it.

- `unity license status` no longer reports `signedIn: true` with `authMode: "oauth"` when nobody is signed in and the sign-in service refuses the CLI. It now reports the saved sign-in, or signed out when none is saved, matching `unity auth status`.

- `unity test` now refuses a `--junit-output` that is the same file as `--output` on the report folder's own terms, instead of letting the JUnit report overwrite the NUnit one. Names that differ by letter case, the dotless and dotted `i`, or Unicode form are only a candidate, and the file system decides, component by component from the first path component that differs: a spelling that exists while the other does not is a different file, two that exist are compared by file identity (a dangling link, whose target cannot be looked up, is judged by its link entry and never counts as a different file), and two that are absent are probed with a file created under the first name in a scratch folder inside the nearest existing folder, removed again, so nothing is created or removed at the report paths (a probe that cannot be made or read proves nothing and does not refuse the run), and right before the JUnit report is written the two files are compared by identity once more, so a link or an 8.3 short name created by the NUnit write cannot send the JUnit report over it (NTFS through WSL keeps `É` and `É` as two files and folds `ı` into `I`, a folding folder makes `É` and `É` one).

- Commands that talk to a running Unity Editor, such as `unity command`, `unity recompile`, `unity job`, `unity logs --console` and `unity projects audit`, no longer crash when the current directory can't be read or no longer exists (a permission refusal such as macOS blocking a protected folder, or a folder deleted under your shell). They skip it, and with several Editors open ask you to pick one with `--project-path`. A relative `--project-path` needs a readable directory, so pass an absolute one instead.

- `unity projects link cloud` links a project whose `ProjectSettings.asset`, as the Editor wrote it, no longer names a cloud project, where it refused with the stale link from the Hub's project list.

- `unity projects link cloud` now exits with code 7 (14 for a timeout) when Unity's services can't be reached (for example behind a dead proxy), instead of exit 6. Unity Cloud commands whose sign-in step gets no answer from Unity's services now say the services can't be reached, instead of telling you to run `unity auth login`.

- `unity projects clone --vcs uvcs` now registers the project with its Unity Version Control link, workspace location included, even when the workspace cannot be read back from `cm`, so `unity projects unlink vcs` finds the workspace it removes instead of reporting the project as already unlinked or naming the wrong folder.

- `unity projects unlink vcs` on Windows no longer refuses a project at the root of its Unity Version Control workspace just because the path differs in letter case (for example `C:\work\game` against the `c:\work\game` that `cm` reports), so it removes the workspace without `--unlink-workspace`. A project below the workspace root is still refused.

- `unity projects clone --vcs uvcs` of a repository you already cloned into another folder no longer fails with "workspace already exists": the second clone gets a workspace named for its folder (`<repo>_<folder>`), and a name still taken is reported with the `cm workspace delete` command to free it.

- On Windows, `unity projects clone --subpath` registers the project with backslashes only (`C:\dir\Sub\Game`), instead of a path that mixed `\` and `/`.

- `unity projects unlink vcs` treats a project whose registry row records its workspace folder itself (as the Hub writes it) as linked to that folder, instead of refusing with a folder two levels above it or removing the `.plastic` there.

- On Windows, pressing Ctrl-C while a command shows its spinner no longer leaves the Windows Terminal tab and taskbar progress indicator spinning.

- `unity terms status --quiet` now prints the same table as a run without `--quiet`, and prints "No installed Editors found." when no Editor is installed, instead of printing nothing.

- On Windows, `unity hub install` now names the expected publisher, `Unity Technologies SF`, when it refuses an installer whose signature does not check out, instead of the macOS Developer ID.

- On Windows, `UNITY_PAGER=more` and `PAGER=more` now page long listings; the pager lookup also finds `.com` programs.

- A `UNITY_PAGER` or `PAGER` that cannot be found now prints a warning on stderr instead of silently printing the listing unpaged.

- A sign-in refresh that times out, and a service-account token exchange that gets no answer from the network, no longer send a crash report. The messages and exit codes are unchanged.

- `unity vcs doctor`, `hooks`, `status`, `switch`, `sync` and `worktree` now say what to check when git cannot be started (not installed, not on your `PATH`, or not allowed to run), under the new error code `GIT_START_FAILED`.

- `unity context` reports an accounts database it cannot write to with the same message and `LOCAL_STORE_UNWRITABLE` code as `unity auth`.

- A pipeline artifact the build does not list, an accounts database `context` cannot write to and a git that cannot be started no longer send a crash report.

- `unity mcp configure` writes the config file when it sits on a different drive than its temporary copy, which happens when `AppData` is redirected, as with a packaged Claude Desktop. Previously it failed with "The system cannot move the file to a different disk drive" and left the config untouched.

- `unity mcp configure` now exits with code 1, instead of 6, when it can't write a config file (for example a read-only folder) in the human, `tsv` and `github` formats; `--format json` and `ndjson` already used 1. Scripts that check for exit 6 on a failed write should check for 1.

- `unity watch test` no longer skips a file change on a busy machine. A slow poll under heavy load used to push the next check far out, so a change could land in the gap and go unnoticed until the following one.

- The one-time privacy notice now matches your analytics setting on every first command. Before, a machine with analytics on could be told how to turn it on instead of how to turn it off.

### Security

- `unity projects close --force` now signals a process only if it is a Unity Editor that did not start after the Editor's recorded start time, and checks again before each signal. A stale or tampered editor record that names a process that is not a Unity Editor, or one that started after the recorded time, is refused with a message, and that process is left running."##;

    #[test]
    fn parses_versions_sections_and_wrapped_entries() {
        let parsed = parse_changelog(SAMPLE);
        assert_eq!(parsed.version.as_deref(), Some("1.0.0-beta.14"));
        assert_eq!(parsed.sections.len(), 5);
        let added = &parsed.sections[0];
        assert_eq!(added.title, "Added");
        assert_eq!(added.entries.len(), 2);
        assert!(added.entries[1].text.contains("second line"));
        assert!(added.entries[0].text.contains("unity config path"));
    }

    #[test]
    fn extraction_prefers_command_sequences_then_classes() {
        let command = classify_entry("- `unity install-modules --architecture arm64` now ...");
        assert_eq!(command.docs_topic.as_deref(), Some("install-modules"));
        assert_eq!(command.docs_kind, DocsTopicKind::Cli);
        assert!(command.app_impact);

        let subcommand = classify_entry("- `unity pipeline editors` lists ...");
        assert_eq!(subcommand.docs_topic.as_deref(), Some("pipeline editors"));
        assert_eq!(subcommand.docs_kind, DocsTopicKind::Cli);

        let class = classify_entry("- `MonoBehaviour.Start` runs before ...");
        assert_eq!(class.docs_topic.as_deref(), Some("MonoBehaviour.Start"));
        assert_eq!(class.docs_kind, DocsTopicKind::ScriptReference);
        assert!(!class.app_impact, "a class name alone is not app impact");

        let both = classify_entry(
            "- `unity install-modules` and `MonoBehaviour.Start` are both mentioned.",
        );
        assert_eq!(both.docs_topic.as_deref(), Some("install-modules"));
        assert_eq!(both.docs_kind, DocsTopicKind::Cli);

        let flags_and_placeholders = classify_entry("- `unity install <version> --architecture`");
        assert_eq!(
            flags_and_placeholders.docs_topic.as_deref(),
            Some("install")
        );
    }

    #[test]
    fn app_impact_uses_commands_flags_and_mapped_codes_only() {
        for relevant in [
            "- `unity docs Start` opens the class page.",
            "- `unity install-modules --architecture arm64` now respects it.",
            "- `--format json` carries the new field.",
            "- It now exits 7 (`VCS_PROVIDER_UNREACHABLE`).",
            "- It exits 2 instead of 1.",
            "- The CLI reports TIMEOUT when the Editor stops answering.",
            "- The target is UNAVAILABLE while it loads.",
        ] {
            assert!(
                classify_entry(relevant).app_impact,
                "{relevant} should affect this app"
            );
        }

        for irrelevant in [
            "- `unity test` now reports faster.",
            "- `--quiet` no longer blanks a table.",
            "- It exits 6 instead of 1.",
            "- `MonoBehaviour.Start` runs before the first frame.",
        ] {
            assert!(
                !classify_entry(irrelevant).app_impact,
                "{irrelevant} should not affect this app"
            );
        }
    }

    #[test]
    fn parsed_entries_carry_filter_signals_and_readable_text() {
        let parsed = parse_changelog(
            "### Added\n\n- `unity install-modules --architecture` now selects a build.\n- A regular note.\n- `--quiet` is unchanged.\n\n### Changed\n\n- `MonoBehaviour.Start` is documented.\n",
        );
        let entries = parsed
            .sections
            .iter()
            .flat_map(|section| section.entries.iter())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4);
        assert!(entries[0].app_impact);
        assert_eq!(entries[0].docs_topic.as_deref(), Some("install-modules"));
        assert!(!entries[1].app_impact);
        assert!(entries[1].docs_topic.is_none());
        assert!(!entries[2].app_impact);
        assert!(!entries[3].app_impact);
        assert_eq!(
            entries[3].docs_topic.as_deref(),
            Some("MonoBehaviour.Start")
        );
        assert_eq!(entries.iter().filter(|entry| entry.app_impact).count(), 1);
        assert!(entries[0].text.contains("now selects a build"));
        assert!(entries[0]
            .text
            .contains("unity install-modules --architecture"));
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

    #[test]
    fn v1_0_1_script_output_changes_are_flagged() {
        let parsed = parse_changelog(V1_0_1);

        for needle in [
            "analytics status",
            "cache key",
            "unity upgrade --format ndjson",
            "captures a table",
            "unity editors path",
            "print-target",
            "completion",
            "regular expression syntax",
            "Numeric options",
            "builds log",
        ] {
            let change = parsed
                .breaking_changes
                .iter()
                .find(|change| change.text.contains(needle))
                .unwrap_or_else(|| panic!("{needle} should be flagged"));
            assert!(
                matches!(
                    change.reason.as_str(),
                    "explicitly marked breaking" | "behavior or output-shape change"
                ),
                "{needle} should be an output or behavior change, got {:?}",
                change.reason
            );
        }
    }

    #[test]
    fn beta17_contract_changes_are_flagged() {
        let parsed = parse_changelog(BETA17);

        for needle in [
            "link vcs --rollback",
            "mcp configure",
            "VCS_PROVIDER_UNREACHABLE",
            "cloud-project",
            "too many arguments",
            "now respects --architecture",
        ] {
            let change = parsed
                .breaking_changes
                .iter()
                .find(|change| change.text.contains(needle))
                .unwrap_or_else(|| panic!("{needle} should be flagged"));
            assert_eq!(change.reason, "behavior or output-shape change");
        }
    }

    #[test]
    fn v1_0_1_additive_and_fix_entries_stay_quiet() {
        let parsed = parse_changelog(V1_0_1);

        for needle in [
            "every command a `path` key",
            "sort names by code point",
            "`unity ci init` no longer refuses",
            "ProjectVersion.txt",
            "promoted but not yet published",
        ] {
            assert!(
                !parsed
                    .breaking_changes
                    .iter()
                    .any(|change| change.text.contains(needle)),
                "{needle} is additive or a fix, not a breaking change"
            );
        }
    }

    #[test]
    fn beta17_telemetry_and_fix_noise_stays_quiet() {
        let parsed = parse_changelog(BETA17);

        for needle in [
            // The exit code explicitly stays the same: no contract change.
            "no longer sends a crash report",
            // Internal paging, pagination and privacy fixes.
            "UNITY_PAGER=more",
            "privacy notice",
            // Security hardening that changes no output or exit code.
            "projects close --force",
            // A fix to how a watch polls files.
            "watch test",
        ] {
            assert!(
                !parsed
                    .breaking_changes
                    .iter()
                    .any(|change| change.text.contains(needle)),
                "{needle} is not a contract change"
            );
        }
    }
}
