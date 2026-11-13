//! Opt-in trace of the CLI processes the app starts.
//!
//! Prompts from the Linux credential store (Secret Service / keyring) belong to
//! whichever `unity` process reads it, so diagnosing "a prompt appeared five
//! times" needs a record of what ran, when, and how long it took. Set
//! `UNITY_HUB_CLI_TRACE=1` and reproduce; lines are appended to
//! `cli-trace.log` next to the CLI install receipt. `UNITY_HUB_CLI_TRACE_FILE`
//! overrides the destination.
//!
//! The log records the resolved command line, which never carries a secret:
//! the app passes no tokens or passwords as arguments, and only
//! `DO_NOT_TRACK=1` is ever placed in the child environment.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const TRACE_ENV: &str = "UNITY_HUB_CLI_TRACE";
const TRACE_FILE_ENV: &str = "UNITY_HUB_CLI_TRACE_FILE";
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// True when the user opted into the trace.
pub fn enabled() -> bool {
    match std::env::var(TRACE_ENV) {
        Ok(value) => {
            let value = value.trim();
            !value.is_empty()
                && value != "0"
                && !value.eq_ignore_ascii_case("false")
                && !value.eq_ignore_ascii_case("off")
        }
        Err(_) => false,
    }
}

fn trace_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(TRACE_FILE_ENV) {
        return Some(PathBuf::from(path));
    }
    Some(dirs::config_dir()?.join("unityhub").join("cli-trace.log"))
}

/// Appends one invocation line. Does nothing unless tracing is enabled.
pub fn record(source: &str, argv: &str, elapsed: Duration, exit_code: Option<i32>) {
    if !enabled() {
        return;
    }
    if let Some(path) = trace_path() {
        let _ = append(&path, record_line(source, argv, elapsed, exit_code));
    }
}

fn record_line(source: &str, argv: &str, elapsed: Duration, exit_code: Option<i32>) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    let exit = exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "-".to_string());
    format!(
        "{} source={} exit={} elapsed_ms={} argv={}\n",
        civil_time(millis),
        source,
        exit,
        elapsed.as_millis(),
        argv
    )
}

fn append(path: &Path, line: String) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0) > MAX_BYTES {
        let mut rotated = path.as_os_str().to_owned();
        rotated.push(".1");
        let rotated = PathBuf::from(rotated);
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(path, &rotated);
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())
}

/// UTC `YYYY-MM-DDTHH:MM:SS.mmmZ`, so a prompt seen at a wall-clock time can be
/// matched to an invocation without asking the reader to decode epoch millis.
fn civil_time(unix_millis: u128) -> String {
    let seconds = (unix_millis / 1000) as i64;
    let millis = unix_millis % 1000;

    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3600;
    let minute = (seconds_of_day % 3600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Howard Hinnant's days-to-civil-date algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    } as u32;
    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_instants() {
        assert_eq!(civil_time(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(civil_time(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
        assert_eq!(civil_time(1_760_000_000_000), "2025-10-09T08:53:20.000Z");
    }

    #[test]
    fn record_line_carries_source_exit_and_argv() {
        let line = record_line(
            "read",
            "unity version --format json",
            Duration::from_millis(42),
            Some(0),
        );
        assert!(line.contains("source=read"));
        assert!(line.contains("exit=0"));
        assert!(line.contains("elapsed_ms=42"));
        assert!(line.contains("argv=unity version --format json"));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn trace_is_disabled_by_default_and_enabled_by_truthy_values() {
        // The environment is process-global, so restore whatever was there.
        let previous = std::env::var_os(TRACE_ENV);

        for value in ["", "0", "false", "off"] {
            std::env::set_var(TRACE_ENV, value);
            assert!(!enabled(), "{value:?} should disable the trace");
        }
        for value in ["1", "yes", "true"] {
            std::env::set_var(TRACE_ENV, value);
            assert!(enabled(), "{value:?} should enable the trace");
        }

        match previous {
            Some(value) => std::env::set_var(TRACE_ENV, value),
            None => std::env::remove_var(TRACE_ENV),
        }
    }

    #[test]
    fn append_rotates_at_the_size_limit() {
        let dir =
            std::env::temp_dir().join(format!("unity-rust-hub-trace-test-{}", std::process::id()));
        let path = dir.join("cli-trace.log");
        let _ = std::fs::remove_dir_all(&dir);

        append(&path, "first\n".to_string()).unwrap();
        std::fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        append(&path, "second\n".to_string()).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second\n");
        let rotated = format!("{}.1", path.display());
        assert!(Path::new(&rotated).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
