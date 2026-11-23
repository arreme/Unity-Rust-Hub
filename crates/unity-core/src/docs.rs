//! The `unity docs` adapter: one resolved documentation page per topic.
//!
//! The CLI owns topic resolution (including the `Start` → `MonoBehaviour.Start`
//! mapping and the search fallback for ambiguous names). This module only
//! builds the read, parses the answer, and refuses to hand the UI a URL that
//! is not HTTP(S); launching a browser is the UI's click, never a lookup.

use serde_json::Value;

use crate::errors::UnityError;

/// Code used when the CLI answers successfully but resolves no page. The
/// controller renders this as the quiet "no docs page found" state instead of
/// a failure.
pub const DOCS_NOT_FOUND: &str = "DOCS_NOT_FOUND";

/// Code used when the CLI answers with something that is not an HTTP(S) URL.
/// The OS opener must never receive another scheme from this app.
pub const DOCS_UNSAFE_URL: &str = "DOCS_UNSAFE_URL";

/// The documentation collection a lookup searches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DocsScope {
    /// The script reference, which is what `unity docs <topic>` uses by
    /// default. No scope flag is passed.
    ScriptReference,
    Manual,
    Cli,
    Hub,
    Cloud,
    Package(String),
}

impl DocsScope {
    /// The flag this scope adds to `unity docs`, if any. `--package <name>` is
    /// returned as two argv elements.
    pub fn args(&self) -> Vec<String> {
        match self {
            Self::ScriptReference => Vec::new(),
            Self::Manual => vec![crate::surface::flag::MANUAL.to_string()],
            Self::Cli => vec![crate::surface::flag::CLI.to_string()],
            Self::Hub => vec![crate::surface::flag::HUB.to_string()],
            Self::Cloud => vec![crate::surface::flag::CLOUD.to_string()],
            Self::Package(name) => vec![
                crate::surface::flag::PACKAGE.to_string(),
                name.trim().to_string(),
            ],
        }
    }
}

/// A docs request. `topic` is already a candidate extracted from an entry
/// (`install-modules`, `pipeline editors`, `MonoBehaviour.Start`), never the
/// release-note sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsLookup {
    pub topic: String,
    pub scope: DocsScope,
}

impl DocsLookup {
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            scope: DocsScope::ScriptReference,
        }
    }

    pub fn with_scope(mut self, scope: DocsScope) -> Self {
        self.scope = scope;
        self
    }
}

/// One resolved page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsPage {
    pub topic: String,
    pub url: String,
    /// The CLI resolved ownership ambiguously and returned a search page.
    pub search: bool,
}

impl DocsPage {
    fn new(topic: &str, url: String, search: bool) -> Result<Self, UnityError> {
        let url = url.trim().to_string();
        if url.is_empty() {
            return Err(not_found(topic));
        }
        if !is_safe_docs_url(&url) {
            return Err(UnityError::new(
                DOCS_UNSAFE_URL,
                "The Unity CLI returned a documentation link this app will not open.",
            )
            .with_detail(format!("Refused URL: {url}")));
        }
        Ok(Self {
            topic: topic.to_string(),
            url,
            search,
        })
    }
}

/// The single URL-safety policy shared by the CLI adapter and the OS opener.
///
/// Only HTTP(S) links may reach a browser: a docs lookup returns URLs from the
/// CLI, and a future CLI or a corrupted answer must not turn into `file://` or
/// an application scheme the user did not click intentionally. The controller
/// calls this same predicate before spawning a platform opener.
pub fn is_safe_docs_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return false;
    }
    !url.chars().any(|character| character.is_control())
}

fn not_found(topic: &str) -> UnityError {
    UnityError::new(
        DOCS_NOT_FOUND,
        format!("No documentation page was found for `{topic}`."),
    )
    .with_detail(
        "Try another entry, a different docs scope, or open the script reference directly.",
    )
}

/// Parses the `data` field of `unity docs <topic> --url --format json`.
///
/// Beta.17 prints the resolved URL (or a search URL). Older/other shapes are
/// accepted defensively: a bare string, `url`/`href`/`link`/`searchUrl`, and a
/// nested `result`, `page`, or `data` object. A missing URL is a quiet
/// [`DOCS_NOT_FOUND`], not a parse failure the user has to decode.
pub fn docs_page_from_data(data: Option<Value>, topic: &str) -> Result<DocsPage, UnityError> {
    let Some(data) = data else {
        return Err(not_found(topic));
    };
    let Some((url, search)) = find_url(&data, 0) else {
        return Err(not_found(topic));
    };
    DocsPage::new(topic, url, search)
}

fn find_url(value: &Value, depth: usize) -> Option<(String, bool)> {
    if depth > 3 {
        return None;
    }
    match value {
        Value::String(url) => Some((url.clone(), looks_like_search_url(url))),
        Value::Object(object) => {
            let explicit = object
                .get("search")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || object
                    .get("isSearch")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                || object
                    .get("resolved")
                    .and_then(Value::as_bool)
                    .map(|resolved| !resolved)
                    .unwrap_or(false);
            let kind_is_search = ["kind", "type"]
                .iter()
                .filter_map(|key| object.get(*key).and_then(Value::as_str))
                .any(|kind| kind.eq_ignore_ascii_case("search"));

            // A search URL has its own key so the UI can say why it is a
            // search rather than an exact page.
            for key in ["searchUrl", "search_url"] {
                if let Some(Value::String(url)) = object.get(key) {
                    if !url.trim().is_empty() {
                        return Some((url.clone(), true));
                    }
                }
            }
            if let Some(Value::String(url)) = object.get("search") {
                if !url.trim().is_empty() {
                    return Some((url.clone(), true));
                }
            }

            for key in [
                "url",
                "pageUrl",
                "page_url",
                "href",
                "link",
                "page",
                "documentationUrl",
            ] {
                if let Some(Value::String(url)) = object.get(key) {
                    if !url.trim().is_empty() {
                        return Some((
                            url.clone(),
                            explicit || kind_is_search || looks_like_search_url(url),
                        ));
                    }
                }
            }

            for key in ["result", "page", "data", "documentation"] {
                if let Some(nested) = object.get(key) {
                    if let Some((url, search)) = find_url(nested, depth + 1) {
                        return Some((url, search || explicit || kind_is_search));
                    }
                }
            }
            None
        }
        Value::Array(items) => items.first().and_then(|item| find_url(item, depth + 1)),
        _ => None,
    }
}

fn looks_like_search_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("/search") || lower.contains("search?") || lower.contains("?q=")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_resolved_url() {
        let page = docs_page_from_data(
            Some(json!({ "url": "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/MonoBehaviour.Start.html" })),
            "MonoBehaviour.Start",
        )
        .unwrap();
        assert_eq!(page.topic, "MonoBehaviour.Start");
        assert!(!page.search);
        assert!(page.url.contains("MonoBehaviour.Start.html"));
    }

    #[test]
    fn parses_a_search_fallback_without_failing() {
        let page = docs_page_from_data(
            Some(json!({
                "resolved": false,
                "searchUrl": "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/30_search.html?q=Start"
            })),
            "Start",
        )
        .unwrap();
        assert!(page.search);
        assert!(page.url.contains("search"));
    }

    #[test]
    fn parses_a_bare_string_and_a_nested_result() {
        let page = docs_page_from_data(
            Some(json!(
                "https://docs.unity3d.com/Manual/InstallingUnity.html"
            )),
            "InstallingUnity",
        )
        .unwrap();
        assert!(!page.search);

        let page = docs_page_from_data(
            Some(json!({ "result": { "url": "https://docs.unity3d.com/Manual/index.html" } })),
            "Manual",
        )
        .unwrap();
        assert!(page.url.ends_with("/index.html"));
    }

    #[test]
    fn a_missing_url_is_docs_not_found() {
        for data in [None, Some(json!(null)), Some(json!({ "topic": "Start" }))] {
            let error = docs_page_from_data(data, "Start").unwrap_err();
            assert_eq!(error.code, DOCS_NOT_FOUND);
        }
    }

    #[test]
    fn refuses_non_http_urls() {
        let error =
            docs_page_from_data(Some(json!({ "url": "file:///etc/passwd" })), "Start").unwrap_err();
        assert_eq!(error.code, DOCS_UNSAFE_URL);
    }

    #[test]
    fn scope_flags_are_stable() {
        assert!(DocsScope::ScriptReference.args().is_empty());
        assert_eq!(
            DocsScope::Cli.args(),
            vec![crate::surface::flag::CLI.to_string()]
        );
        assert_eq!(
            DocsScope::Package("com.unity.textmeshpro".to_string()).args(),
            vec![
                crate::surface::flag::PACKAGE.to_string(),
                "com.unity.textmeshpro".to_string()
            ]
        );
    }
}
