//! The Unity CLI surface this app drives.
//!
//! Command builders in [`crate::runner`] take their command and flag tokens
//! from the constants below, and the changelog app-impact filter matches
//! against the same registry. There is deliberately no second hand-written
//! list in the controller or the UI: adding a command the app runs starts here.
//!
//! The registry names the surface the app is built against, including pages
//! that land later on the same branch (plugins, skills, cloud builds,
//! templates). A command is only in this list because a screen or adapter
//! method either calls it already or is being added in this release; the
//! changelog filter treats an entry mentioning any of them as relevant.

/// One command in the registry. `subcommands` are the second-level tokens the
/// app knows how to name; an unknown second token still counts as a mention of
/// the command, but the docs topic is the bare command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSurface {
    pub name: &'static str,
    pub subcommands: &'static [&'static str],
}

/// Command tokens. Every builder in `runner.rs` uses these instead of a
/// literal so the filter cannot drift from what the app actually runs.
pub mod command {
    pub const VERSION: &str = "version";
    pub const INSTALL_PATH: &str = "install-path";
    pub const EDITORS: &str = "editors";
    pub const PIPELINE: &str = "pipeline";
    pub const CHANGELOG: &str = "changelog";
    pub const AUTH: &str = "auth";
    pub const LICENSE: &str = "license";
    pub const DOCTOR: &str = "doctor";
    pub const ENV: &str = "env";
    pub const DIAGNOSE: &str = "diagnose";
    pub const INSTALL: &str = "install";
    pub const INSTALL_MODULES: &str = "install-modules";
    pub const MODULES: &str = "modules";
    pub const RELEASES: &str = "releases";
    pub const PROJECTS: &str = "projects";
    pub const OPEN: &str = "open";
    pub const SELF_UPDATE: &str = "self-update";
    pub const DOCS: &str = "docs";
    pub const STATUS: &str = "status";
    pub const PLUGIN: &str = "plugin";
    pub const SKILL: &str = "skill";
    pub const TEMPLATES: &str = "templates";
    pub const CLOUD: &str = "cloud";
    pub const BUILD: &str = "build";
}

/// Second-level tokens used both for command paths and docs topics.
pub mod subcommand {
    pub const INSTALL: &str = "install";
    pub const INSTALLED: &str = "installed";
    pub const EDITORS: &str = "editors";
    pub const RUNNING: &str = "running";
    pub const INFO: &str = "info";
    pub const DEFAULT: &str = "default";
    pub const VERIFY: &str = "verify";
    pub const PRUNE: &str = "prune";
    pub const STATUS: &str = "status";
    pub const LOGIN: &str = "login";
    pub const LOGOUT: &str = "logout";
    pub const PROXY: &str = "proxy";
    pub const ACCELERATOR: &str = "accelerator";
    pub const UPDATE: &str = "update";
    pub const UPGRADE: &str = "upgrade";
    pub const CREATE: &str = "create";
    pub const CLONE: &str = "clone";
    pub const LINK: &str = "link";
    pub const UNLINK: &str = "unlink";
    pub const AUDIT: &str = "audit";
    pub const CACHE: &str = "cache";
    pub const REFRESH: &str = "refresh";
    pub const REMOVE: &str = "remove";
    pub const LIST: &str = "list";
    pub const CLOUD_BUILD: &str = "cloud-build";
}

/// Flag tokens the app passes to the CLI.
pub mod flag {
    pub const FORMAT: &str = "--format";
    pub const NO_PAGER: &str = "--no-pager";
    pub const NO_BANNER: &str = "--no-banner";
    pub const NON_INTERACTIVE: &str = "--non-interactive";
    pub const INSTALLED: &str = "--installed";
    pub const LONG: &str = "--long";
    pub const TARGET: &str = "--target";
    pub const ARCHITECTURE: &str = "--architecture";
    pub const MODULE: &str = "-m";
    pub const HEADLESS: &str = "--headless";
    pub const ACCEPT_EULA: &str = "--accept-eula";
    pub const RESUME: &str = "--resume";
    pub const DOWNLOAD_ONLY: &str = "--download-only";
    pub const NO_WAIT: &str = "--no-wait";
    pub const TIMEOUT: &str = "--timeout";
    pub const SET_DEFAULT: &str = "--set-default";
    pub const YES: &str = "--yes";
    pub const RETRIES: &str = "--retries";
    pub const DOWNLOAD_DIRECTORY: &str = "--download-directory";
    pub const UNSET: &str = "--unset";
    pub const REMOVE: &str = "--remove";
    pub const REMOVE_MISSING: &str = "--remove-missing";
    pub const FORCE: &str = "--force";
    pub const STREAM: &str = "--stream";
    pub const LIMIT: &str = "--limit";
    pub const OFFSET: &str = "--offset";
    pub const TO: &str = "--to";
    pub const CHECK: &str = "--check";
    pub const CHANGELOG: &str = "--changelog";
    pub const DRY_RUN: &str = "--dry-run";
    pub const ROLLBACK: &str = "--rollback";
    pub const CHANNEL: &str = "--channel";
    pub const ALL: &str = "--all";
    pub const URL: &str = "--url";
    pub const MANUAL: &str = "--manual";
    pub const CLI: &str = "--cli";
    pub const HUB: &str = "--hub";
    pub const CLOUD: &str = "--cloud";
    pub const PACKAGE: &str = "--package";
    pub const EDITOR_VERSION: &str = "--editor-version";
    pub const PROJECT_PATH: &str = "--project-path";
    pub const GIT_TOKEN_FROM_STDIN: &str = "--git-token-from-stdin";
}

/// Stable CLI error codes the app turns into remediation actions. The codes
/// are declared here so the changelog filter and `errors::code_remediation`
/// consume one list.
pub mod error_code {
    pub const AUTH_ERROR: &str = "AUTH_ERROR";
    pub const NOT_SIGNED_IN: &str = "NOT_SIGNED_IN";
    pub const SESSION_EXPIRED: &str = "SESSION_EXPIRED";
    pub const NETWORK_UNREACHABLE: &str = "NETWORK_UNREACHABLE";
    pub const VCS_PROVIDER_UNREACHABLE: &str = "VCS_PROVIDER_UNREACHABLE";
    pub const INSTALL_LOCKED: &str = "INSTALL_LOCKED";
    pub const RELEASE_NOT_FOUND: &str = "RELEASE_NOT_FOUND";
    pub const EDITOR_UNREACHABLE: &str = "EDITOR_UNREACHABLE";
    pub const TIMEOUT: &str = "TIMEOUT";
    pub const EDITOR_TIMEOUT: &str = "EDITOR_TIMEOUT";
    pub const UNAVAILABLE: &str = "UNAVAILABLE";
    pub const STATUS_SAFE_MODE: &str = "STATUS_SAFE_MODE";
    pub const AWAITING_INPUT: &str = "AWAITING_INPUT";
    pub const VCS_TOKEN_REQUIRED: &str = "VCS_TOKEN_REQUIRED";
    pub const GIT_START_FAILED: &str = "GIT_START_FAILED";
    pub const LOCAL_STORE_UNWRITABLE: &str = "LOCAL_STORE_UNWRITABLE";
    pub const LINK_VCS_DELETE_REMOTE_REQUIRES_YES: &str = "LINK_VCS_DELETE_REMOTE_REQUIRES_YES";
    pub const INSTALL_HEADLESS_UNAVAILABLE: &str = "INSTALL_HEADLESS_UNAVAILABLE";
    pub const BAD_ARGS: &str = "BAD_ARGS";
}

/// Commands in the registry, in display order.
pub const COMMANDS: &[CommandSurface] = &[
    CommandSurface {
        name: command::VERSION,
        subcommands: &[],
    },
    CommandSurface {
        name: command::INSTALL_PATH,
        subcommands: &[],
    },
    CommandSurface {
        name: command::EDITORS,
        subcommands: &[
            subcommand::INSTALLED,
            subcommand::RUNNING,
            subcommand::INFO,
            subcommand::DEFAULT,
            subcommand::VERIFY,
            subcommand::PRUNE,
        ],
    },
    CommandSurface {
        name: command::PIPELINE,
        subcommands: &[subcommand::EDITORS, subcommand::CLOUD_BUILD],
    },
    CommandSurface {
        name: command::CHANGELOG,
        subcommands: &[],
    },
    CommandSurface {
        name: command::AUTH,
        subcommands: &[subcommand::STATUS, subcommand::LOGIN, subcommand::LOGOUT],
    },
    CommandSurface {
        name: command::LICENSE,
        subcommands: &[subcommand::STATUS],
    },
    CommandSurface {
        name: command::DOCTOR,
        subcommands: &[],
    },
    CommandSurface {
        name: command::ENV,
        subcommands: &[],
    },
    CommandSurface {
        name: command::DIAGNOSE,
        subcommands: &[
            subcommand::PROXY,
            subcommand::ACCELERATOR,
            subcommand::UPDATE,
        ],
    },
    CommandSurface {
        name: command::INSTALL,
        subcommands: &[],
    },
    CommandSurface {
        name: command::INSTALL_MODULES,
        subcommands: &[],
    },
    CommandSurface {
        name: command::MODULES,
        subcommands: &[],
    },
    CommandSurface {
        name: command::RELEASES,
        subcommands: &[],
    },
    CommandSurface {
        name: command::PROJECTS,
        subcommands: &[
            subcommand::UPGRADE,
            subcommand::CREATE,
            subcommand::CLONE,
            subcommand::LINK,
            subcommand::UNLINK,
            subcommand::INFO,
            subcommand::AUDIT,
        ],
    },
    CommandSurface {
        name: command::OPEN,
        subcommands: &[],
    },
    CommandSurface {
        name: command::SELF_UPDATE,
        subcommands: &[],
    },
    CommandSurface {
        name: command::DOCS,
        subcommands: &[subcommand::CACHE],
    },
    CommandSurface {
        name: command::STATUS,
        subcommands: &[],
    },
    CommandSurface {
        name: command::PLUGIN,
        subcommands: &[
            subcommand::INSTALL,
            subcommand::UPGRADE,
            subcommand::REMOVE,
            subcommand::LIST,
        ],
    },
    CommandSurface {
        name: command::SKILL,
        subcommands: &[subcommand::INSTALL, subcommand::REFRESH],
    },
    CommandSurface {
        name: command::TEMPLATES,
        subcommands: &[],
    },
    CommandSurface {
        name: command::CLOUD,
        subcommands: &[],
    },
    CommandSurface {
        name: command::BUILD,
        subcommands: &[],
    },
];

/// Every flag the app passes, in the order it appears in commands.
pub const FLAGS: &[&str] = &[
    flag::FORMAT,
    flag::NO_PAGER,
    flag::NO_BANNER,
    flag::NON_INTERACTIVE,
    flag::INSTALLED,
    flag::LONG,
    flag::TARGET,
    flag::ARCHITECTURE,
    flag::MODULE,
    flag::HEADLESS,
    flag::ACCEPT_EULA,
    flag::RESUME,
    flag::DOWNLOAD_ONLY,
    flag::NO_WAIT,
    flag::TIMEOUT,
    flag::SET_DEFAULT,
    flag::YES,
    flag::RETRIES,
    flag::DOWNLOAD_DIRECTORY,
    flag::UNSET,
    flag::REMOVE,
    flag::REMOVE_MISSING,
    flag::FORCE,
    flag::STREAM,
    flag::LIMIT,
    flag::OFFSET,
    flag::TO,
    flag::CHECK,
    flag::CHANGELOG,
    flag::DRY_RUN,
    flag::ROLLBACK,
    flag::CHANNEL,
    flag::ALL,
    flag::URL,
    flag::MANUAL,
    flag::CLI,
    flag::HUB,
    flag::CLOUD,
    flag::PACKAGE,
    flag::EDITOR_VERSION,
    flag::PROJECT_PATH,
    flag::GIT_TOKEN_FROM_STDIN,
];

/// Stable error codes the app maps to a remediation action.
pub const ERROR_CODES: &[&str] = &[
    error_code::AUTH_ERROR,
    error_code::NOT_SIGNED_IN,
    error_code::SESSION_EXPIRED,
    error_code::NETWORK_UNREACHABLE,
    error_code::VCS_PROVIDER_UNREACHABLE,
    error_code::INSTALL_LOCKED,
    error_code::RELEASE_NOT_FOUND,
    error_code::EDITOR_UNREACHABLE,
    error_code::TIMEOUT,
    error_code::EDITOR_TIMEOUT,
    error_code::UNAVAILABLE,
    error_code::STATUS_SAFE_MODE,
    error_code::AWAITING_INPUT,
    error_code::VCS_TOKEN_REQUIRED,
    error_code::GIT_START_FAILED,
    error_code::LOCAL_STORE_UNWRITABLE,
    error_code::LINK_VCS_DELETE_REMOTE_REQUIRES_YES,
    error_code::INSTALL_HEADLESS_UNAVAILABLE,
    error_code::BAD_ARGS,
];

/// Exit codes the app has an explicit remedy for. Changing this list also
/// changes the matching table in `errors::exit_remediation`.
pub const MAPPED_EXIT_CODES: &[i32] = &[2, 3, 7, 9, 11, 12, 14, 17, 18];

pub fn is_command(token: &str) -> bool {
    COMMANDS.iter().any(|command| command.name == token)
}

pub fn is_flag(token: &str) -> bool {
    FLAGS.contains(&token)
}

pub fn is_error_code(token: &str) -> bool {
    ERROR_CODES.contains(&token)
}

pub fn is_mapped_exit_code(code: i32) -> bool {
    MAPPED_EXIT_CODES.contains(&code)
}

/// Resolves `unity <command> [<subcommand>]` (or a bare command span) to the
/// command path the CLI docs index uses. A leading `unity` is optional, so a
/// backticked `` `install-modules --architecture` `` is recognised too.
///
/// An unknown command returns `None`; an unknown second token still returns
/// the bare command. Argument values and flags never become part of the path.
pub fn command_path(tokens: &[&str]) -> Option<String> {
    let mut tokens = tokens;
    if tokens
        .first()
        .is_some_and(|token| token.eq_ignore_ascii_case("unity"))
    {
        tokens = &tokens[1..];
    }
    let name = *tokens.first()?;
    let command = COMMANDS.iter().find(|command| command.name == name)?;

    let mut path = command.name.to_string();
    if let Some(second) = tokens.get(1) {
        if command.subcommands.contains(second) {
            path.push(' ');
            path.push_str(second);
        }
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_paths_ignore_flags_and_arguments() {
        assert_eq!(
            command_path(&["unity", "install-modules", "--architecture", "arm64"]).as_deref(),
            Some("install-modules")
        );
        assert_eq!(
            command_path(&["unity", "pipeline", "editors"]).as_deref(),
            Some("pipeline editors")
        );
        assert_eq!(
            command_path(&["editors", "default", "6000.3.15f1"]).as_deref(),
            Some("editors default")
        );
        assert_eq!(
            command_path(&["unity", "projects", "audit", "/tmp/Game"]).as_deref(),
            Some("projects audit")
        );
    }

    #[test]
    fn unknown_commands_are_not_surface() {
        assert!(command_path(&["unity", "definitely-not-a-command"]).is_none());
        assert!(!is_command("definitely-not-a-command"));
        assert!(!is_flag("--quiet"));
        assert!(!is_error_code("SOMETHING_ELSE"));
    }

    #[test]
    fn every_error_code_has_a_remediation() {
        for code in ERROR_CODES {
            let error = crate::errors::UnityError::new(*code, "test");
            assert!(
                crate::errors::remediation_for(&error, None).is_some(),
                "{code} is in the surface but has no remediation"
            );
        }
    }

    #[test]
    fn every_mapped_exit_code_has_a_remediation() {
        for code in MAPPED_EXIT_CODES {
            let error = crate::errors::UnityError::new("ERROR", "test").with_exit_code(Some(*code));
            assert!(
                crate::errors::remediation_for(&error, None).is_some(),
                "exit code {code} is in the surface but has no remediation"
            );
        }
    }
}
