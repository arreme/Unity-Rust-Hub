//! Process creation policy shared by every child this app starts.
//!
//! The release binary is a GUI-subsystem executable
//! (`windows_subsystem = "windows"`), so it owns no console. `unity.exe` is a
//! console-subsystem executable. On Windows, creating a console child from a
//! parent without a console makes the OS allocate a brand-new console window
//! for it; the window is visible for as long as the child lives. Since every
//! page read is a short-lived CLI invocation, the effect is a terminal window
//! that flashes on each tab open and tab switch — one per CLI call in the
//! page's read batch.
//!
//! `CREATE_NO_WINDOW` keeps the console the child may need (it still has
//! valid standard handles and an invisible console) while never showing a
//! window. Every spawn path must go through [`hidden`]/[`hide`] so no code
//! path forgets the flag.

use std::ffi::OsStr;
use std::process::Command;

/// `CREATE_NO_WINDOW` from `winbase.h`.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Builds a [`Command`] for `program` that never opens a console window on
/// Windows. Use this instead of `Command::new` for every CLI or shell child.
pub fn hidden(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide(&mut command);
    command
}

/// Applies the no-window creation policy to an already-built command.
pub fn hide(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        command.creation_flags(CREATE_NO_WINDOW);
    }

    #[cfg(not(windows))]
    {
        // Console windows only exist on Windows; nothing to do elsewhere.
        let _ = command;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_commands_still_capture_output() {
        #[cfg(windows)]
        let mut command = hidden("cmd");
        #[cfg(windows)]
        command.args(["/C", "echo", "probe"]);

        #[cfg(not(windows))]
        let mut command = hidden("sh");
        #[cfg(not(windows))]
        command.args(["-c", "echo probe"]);

        let output = command.output().expect("child should start");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "probe");
    }

    /// The regression this module exists for: a console child spawned from a
    /// console host inherits that console, which is what makes Windows allocate
    /// a visible window when the host is a GUI app. A child created through
    /// [`hidden`] gets its own, windowless console instead.
    ///
    /// `GetConsoleProcessList` is used rather than `GetConsoleWindow` because
    /// headless hosts report a null window handle for a console that exists.
    #[cfg(windows)]
    #[test]
    fn hidden_children_do_not_inherit_the_parent_console() {
        let mut visible = Command::new("powershell");
        let Some(inherited) = console_processes(&mut visible) else {
            return; // no usable PowerShell; nothing to probe with
        };
        let me = std::process::id();
        if !inherited.contains(&me) {
            // This host has no console to inherit (detached CI runner), so the
            // flag has no observable effect here.
            eprintln!("skipping: the test host has no inheritable console");
            return;
        }

        let mut hidden_command = hidden("powershell");
        let processes = console_processes(&mut hidden_command).expect("hidden child should run");
        assert!(
            !processes.contains(&me),
            "hidden child stayed on the parent console: {processes:?}"
        );
    }

    #[cfg(windows)]
    fn console_processes(command: &mut Command) -> Option<Vec<u32>> {
        let script = "Add-Type -Name K -Namespace P -MemberDefinition \
             '[DllImport(\"kernel32.dll\")] public static extern uint GetConsoleProcessList(uint[] list, uint count);'; \
             $l = New-Object 'uint32[]' 16; $n = [P.K]::GetConsoleProcessList($l, 16); \
             'pids=' + (($l[0..($n - 1)]) -join ',')";
        let output = command
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let list = text.trim().strip_prefix("pids=")?;
        Some(
            list.split(',')
                .filter_map(|pid| pid.trim().parse().ok())
                .collect(),
        )
    }
}
