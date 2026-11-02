use std::borrow::Cow;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
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
}
