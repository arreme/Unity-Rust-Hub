use std::borrow::Cow;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::{Command, Stdio};
use std::sync::{mpsc, OnceLock};

use chrono::{DateTime, Utc};
use unity_core::UnityError;

type ReadJob = Box<dyn FnOnce() + Send + 'static>;

fn read_queue() -> &'static mpsc::Sender<ReadJob> {
    static QUEUE: OnceLock<mpsc::Sender<ReadJob>> = OnceLock::new();

    QUEUE.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<ReadJob>();
        std::thread::Builder::new()
            .name("cli-reader".to_string())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let _ = catch_unwind(AssertUnwindSafe(job));
                }
            })
            .expect("failed to spawn the CLI reader thread");
        sender
    })
}

/// Runs blocking CLI work on the single reader worker and returns a future the
/// iced executor can await. Serializing reads keeps only one `unity` process
/// (and its ~25-35 MB working set) alive at a time.
pub fn run_blocking<T, F>(work: F) -> impl Future<Output = Result<T, UnityError>>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, UnityError> + Send + 'static,
{
    let (sender, receiver) = futures::channel::oneshot::channel();

    let job: ReadJob = Box::new(move || {
        let result = catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| {
            Err(UnityError::new(
                "INTERNAL",
                "A background CLI task panicked.",
            ))
        });
        let _ = sender.send(result);
    });

    if let Err(error) = read_queue().send(job) {
        // Only possible while shutting down; run the job so its receiver gets
        // an answer instead of hanging.
        (error.0)();
    }

    async move {
        receiver.await.unwrap_or_else(|_| {
            Err(UnityError::new(
                "INTERNAL",
                "A background CLI task ended unexpectedly.",
            ))
        })
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }

    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_epoch_millis(value: Option<i64>) -> String {
    value
        .and_then(DateTime::<Utc>::from_timestamp_millis)
        .map(|date| date.format("%b %d, %Y").to_string())
        .unwrap_or_else(|| "—".to_string())
}

pub fn format_iso(value: Option<&str>) -> String {
    value
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|date| date.format("%b %d, %Y").to_string())
        .unwrap_or_else(|| "—".to_string())
}

pub fn short_path(path: &str, max: usize) -> Cow<'_, str> {
    if path.chars().count() <= max {
        return Cow::Borrowed(path);
    }

    let tail: String = path
        .chars()
        .rev()
        .take(max.saturating_sub(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Cow::Owned(format!("…{tail}"))
}

pub fn error_text(error: &UnityError) -> String {
    match &error.detail {
        Some(detail) if !detail.trim().is_empty() => {
            format!("{}\n\n{}", error.message, detail.trim())
        }
        _ => error.message.clone(),
    }
}

/// One OS-opener invocation. Kept separate from spawning so tests can assert
/// the shape without launching anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenerCommand {
    pub program: &'static str,
    pub args: Vec<String>,
}

/// The ordered candidates for opening `url`. Linux tries the desktop opener
/// first and then two standard fallbacks; Windows and macOS have one each.
///
/// The HTTP(S)-only rule is `unity_core::docs::is_safe_docs_url`, the same
/// predicate the adapter uses before handing a URL to the view model, so the
/// trust boundary has one definition.
pub fn opener_commands(url: &str) -> Result<Vec<OpenerCommand>, String> {
    if !unity_core::docs::is_safe_docs_url(url) {
        return Err("Only http(s) links can be opened.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        return Ok(vec![OpenerCommand {
            program: "cmd",
            args: vec![
                "/C".to_string(),
                "start".to_string(),
                String::new(),
                url.to_string(),
            ],
        }]);
    }

    #[cfg(target_os = "macos")]
    {
        return Ok(vec![OpenerCommand {
            program: "open",
            args: vec![url.to_string()],
        }]);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(vec![
            OpenerCommand {
                program: "xdg-open",
                args: vec![url.to_string()],
            },
            OpenerCommand {
                program: "gio",
                args: vec!["open".to_string(), url.to_string()],
            },
            OpenerCommand {
                program: "sensible-browser",
                args: vec![url.to_string()],
            },
        ])
    }
}

/// Opens a resolved docs URL in the user's browser. The app never calls this
/// during a lookup; it runs only from an explicit click.
pub fn open_external_url(url: &str) -> Result<(), String> {
    let candidates = opener_commands(url)?;
    let mut last_error = None;
    for candidate in candidates {
        match Command::new(candidate.program)
            .args(&candidate.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                // The opener is fire-and-forget, but its child must be reaped
                // or every click leaves a zombie on Unix. Wait on a detached
                // thread so the click still returns immediately.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
            Err(error) => last_error = Some(format!("{}: {error}", candidate.program)),
        }
    }
    Err(format!(
        "No system opener could open the link ({}).",
        last_error.unwrap_or_else(|| "no opener configured".to_string())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn blocking_reads_are_serialized() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let mut futures = Vec::new();
        for _ in 0..2 {
            let active = active.clone();
            let max_active = max_active.clone();
            futures.push(run_blocking(move || {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(30));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }));
        }

        for result in futures::executor::block_on(futures::future::join_all(futures)) {
            result.expect("read should finish");
        }

        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn opener_accepts_http_links_and_refuses_other_schemes() {
        for url in [
            "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/Start.html",
            "http://example.com/docs",
        ] {
            assert!(opener_commands(url).is_ok(), "{url} should be openable");
        }
        for url in ["file:///etc/passwd", "javascript:alert(1)", "not a url", ""] {
            assert!(
                opener_commands(url).is_err(),
                "{url} must never reach the OS opener"
            );
        }
    }

    #[test]
    fn opener_candidate_is_platform_shaped() {
        let commands = opener_commands("https://docs.unity3d.com/").unwrap();
        assert!(!commands.is_empty());
        assert!(commands.iter().all(|command| !command.args.is_empty()));

        #[cfg(target_os = "windows")]
        assert_eq!(commands[0].program, "cmd");
        #[cfg(target_os = "macos")]
        assert_eq!(commands[0].program, "open");
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            assert_eq!(commands[0].program, "xdg-open");
            assert!(commands.iter().any(|command| command.program == "gio"));
        }
    }
}
