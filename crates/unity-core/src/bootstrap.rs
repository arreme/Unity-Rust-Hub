use crate::runner::{svec, CommandSpec};

/// The official installer command, exactly as documented by Unity.
///
/// `UNITY_CLI_SKIP_SETUP=1` is added so the installer does not launch the
/// interactive `unity setup` prompt underneath the GUI, where it would hang
/// waiting for a terminal.
pub fn install_cli_spec(channel: Option<&str>) -> CommandSpec {
    let channel = channel.unwrap_or("stable").trim().to_ascii_lowercase();
    let channel = match channel.as_str() {
        "alpha" | "beta" => channel.as_str(),
        _ => "stable",
    };

    #[cfg(target_os = "windows")]
    let spec = CommandSpec::new(
        "powershell",
        svec(&[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "irm https://unity.com/install.ps1 | iex",
        ]),
    );

    #[cfg(not(target_os = "windows"))]
    let spec = CommandSpec::new(
        "bash",
        svec(&["-c", "curl -fsSL https://unity.com/install.sh | bash"]),
    );

    let spec = spec.with_env("UNITY_CLI_SKIP_SETUP", "1");
    if channel != "stable" {
        spec.with_env("UNITY_CLI_CHANNEL", channel)
    } else {
        spec
    }
}

pub const INSTALLER_URL_LINUX: &str = "https://unity.com/install.sh";
pub const INSTALLER_URL_WINDOWS: &str = "https://unity.com/install.ps1";
