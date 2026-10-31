use std::future::Future;

use chrono::{DateTime, Utc};
use unity_core::UnityError;

/// Runs blocking CLI work on a dedicated thread and returns a future the iced
/// executor can await without blocking the UI.
pub fn run_blocking<T, F>(work: F) -> impl Future<Output = Result<T, UnityError>>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, UnityError> + Send + 'static,
{
    let (sender, receiver) = futures::channel::oneshot::channel();

    std::thread::spawn(move || {
        let _ = sender.send(work());
    });

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

pub fn short_path(path: &str, max: usize) -> String {
    if path.chars().count() <= max {
        return path.to_string();
    }

    let tail: String = path
        .chars()
        .rev()
        .take(max.saturating_sub(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    format!("…{tail}")
}

pub fn error_text(error: &UnityError) -> String {
    match &error.detail {
        Some(detail) if !detail.trim().is_empty() => {
            format!("{}\n\n{}", error.message, detail.trim())
        }
        _ => error.message.clone(),
    }
}
