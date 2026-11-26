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
/// app knows how to name; an unknown second token is an argument only for a
/// command that declares `free_argument`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSurface {
    pub name: &'static str,
    pub subcommands: &'static [&'static str],
    /// When true, a second token that is not a known subcommand is a value
    /// (a docs topic), not an unknown subtree.
    pub free_argument: bool,
}

impl CommandSurface {
    const fn leaf(name: &'static str) -> Self {
        Self {
            name,
            subcommands: &[],
            free_argument: false,
        }
    }

    const fn with_subcommands(name: &'static str, subcommands: &'static [&'static str]) -> Self {
        Self {
            name,
            subcommands,
            free_argument: false,
        }
    }

    const fn with_argument(name: &'static str, subcommands: &'static [&'static str]) -> Self {
        Self {
            name,
            subcommands,
            free_argument: true,
        }
    }
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
    pub const SKILLS: &str = "skills";
    pub const TEMPLATES: &str = "templates";
    pub const CLOUD: &str = "cloud";
    pub const BUILD: &str = "build";
    /// 1.1.0: the app adopts these when the dialog/close and skill flows land.
    pub const CLOSE: &str = "close";
    pub const COMMAND: &str = "command";
    pub const UNINSTALL: &str = "uninstall";
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
    pub const ADD: &str = "add";
    pub const PIN: &str = "pin";
    pub const UNPIN: &str = "unpin";
    pub const CLOSE: &str = "close";
    pub const BROKER: &str = "broker";
    pub const ENSURE: &str = "ensure";
    pub const PACK: &str = "pack";
    pub const SHOW: &str = "show";
    pub const UNINSTALL: &str = "uninstall";
    pub const TOC: &str = "toc";
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
    pub const LIST: &str = "--list";
    pub const LOCAL: &str = "--local";
    pub const SKILL_NAME: &str = "--skill";
    pub const BRIEF: &str = "--brief";
    pub const TYPE: &str = "--type";
    pub const NAME: &str = "--name";
    pub const DISPLAY_NAME: &str = "--display-name";
    pub const DESCRIPTION: &str = "--description";
    pub const OUTPUT_DIR: &str = "--output-dir";
    pub const OUTPUT: &str = "--output";
    pub const OVERWRITE: &str = "--overwrite";
    pub const TEMPLATE: &str = "--template";
    pub const NO_CLOUD: &str = "--no-cloud";
    pub const OPEN: &str = "--open";
    pub const BARE: &str = "--bare";
    pub const VERSION: &str = "--version";
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
    /// 1.1.0: a modal dialog blocked an Editor command or a close.
    pub const EDITOR_BLOCKED_BY_DIALOG: &str = "EDITOR_BLOCKED_BY_DIALOG";
    pub const PROJECTS_CLOSE_BLOCKED_BY_DIALOG: &str = "PROJECTS_CLOSE_BLOCKED_BY_DIALOG";
    /// 1.1.0: a proxy environment variable named a PAC script.
    pub const INVALID_COMMAND_ARGS: &str = "INVALID_COMMAND_ARGS";
    /// 1.1.0 moved these not-found failures from exit 6 to exit 11.
    pub const EDITOR_NOT_INSTALLED: &str = "EDITOR_NOT_INSTALLED";
    pub const NOT_A_PROJECT: &str = "NOT_A_PROJECT";
    pub const PROJECTS_REMOVE_NOT_FOUND: &str = "PROJECTS_REMOVE_NOT_FOUND";
    pub const VCS_TOKEN_REQUIRED: &str = "VCS_TOKEN_REQUIRED";
    pub const GIT_START_FAILED: &str = "GIT_START_FAILED";
    pub const LOCAL_STORE_UNWRITABLE: &str = "LOCAL_STORE_UNWRITABLE";
    pub const LINK_VCS_DELETE_REMOTE_REQUIRES_YES: &str = "LINK_VCS_DELETE_REMOTE_REQUIRES_YES";
    pub const INSTALL_HEADLESS_UNAVAILABLE: &str = "INSTALL_HEADLESS_UNAVAILABLE";
    pub const BAD_ARGS: &str = "BAD_ARGS";
}

/// Stable CLI codes that have no dedicated remediation but still drive the
/// changelog filter and controller branches. Codes with a remediation live in
/// [`error_code`] and are also recognised by [`is_error_code`].
pub mod cli_code {
    pub const COMMAND_FAILED: &str = "COMMAND_FAILED";
    pub const NOT_FOUND: &str = "NOT_FOUND";
    pub const STATUS_NO_INSTANCES: &str = "STATUS_NO_INSTANCES";
    pub const DOWNLOAD_FAILED: &str = "DOWNLOAD_FAILED";
    pub const CHECKSUM_MISMATCH: &str = "CHECKSUM_MISMATCH";
    pub const CANCELLED: &str = "CANCELLED";
    pub const INTERRUPTED: &str = "INTERRUPTED";
    pub const DOCS_NOT_FOUND: &str = "DOCS_NOT_FOUND";
    pub const EDITORS_PRUNE_EDITOR_CHECK_FAILED: &str = "EDITORS_PRUNE_EDITOR_CHECK_FAILED";
    pub const EDITORS_PRUNE_EDITOR_CHECK_UNAVAILABLE: &str =
        "EDITORS_PRUNE_EDITOR_CHECK_UNAVAILABLE";
    /// 1.1.0: `unity command` lost the Editor connection; the command may
    /// have run, so it is not safe to retry blindly.
    pub const EDITOR_CONNECTION_LOST: &str = "EDITOR_CONNECTION_LOST";
    pub const EDITOR_NOT_RUNNING: &str = "EDITOR_NOT_RUNNING";
    pub const EDITOR_NOT_READY: &str = "EDITOR_NOT_READY";
    pub const UNSUPPORTED_STATE: &str = "UNSUPPORTED_STATE";
    pub const TEMPLATE_OTHER_VERSION_ON_DISK: &str = "TEMPLATE_OTHER_VERSION_ON_DISK";
    /// Auth-broker failures the Overview treats as "reads may prompt again".
    pub const AUTH_BROKER_UNAVAILABLE: &str = "AUTH_BROKER_UNAVAILABLE";
    pub const AUTH_BROKER_REFUSED: &str = "AUTH_BROKER_REFUSED";
    pub const CREDENTIAL_STORE_ACCESS_DENIED: &str = "CREDENTIAL_STORE_ACCESS_DENIED";
}

/// Command paths the ignored live suite asserts against the real CLI.
///
/// This lives in the registry so a command rename is a one-line change here,
/// not a second list in `tests/live.rs`.
pub const REQUIRED_COMMAND_PATHS: &[&str] = &[
    "version",
    "install-path",
    "editors",
    "editors running",
    "editors info",
    "editors default",
    "editors verify",
    "editors prune",
    "pipeline editors",
    "status",
    "releases",
    "modules",
    "projects",
    "projects add",
    "projects remove",
    "projects pin",
    "projects unpin",
    "projects create",
    "projects upgrade",
    "templates",
    "templates info",
    "templates create",
    "templates pack",
    "open",
    "changelog",
    "auth status",
    "auth login",
    "auth broker ensure",
    "license status",
    "install",
    "install-modules",
    "self-update",
];

/// Commands in the registry, in display order.
pub const COMMANDS: &[CommandSurface] = &[
    CommandSurface::leaf(command::VERSION),
    CommandSurface::leaf(command::INSTALL_PATH),
    CommandSurface::with_subcommands(
        command::EDITORS,
        &[
            subcommand::INSTALLED,
            subcommand::RUNNING,
            subcommand::INFO,
            subcommand::DEFAULT,
            subcommand::VERIFY,
            subcommand::PRUNE,
        ],
    ),
    CommandSurface::with_subcommands(
        command::PIPELINE,
        &[subcommand::EDITORS, subcommand::CLOUD_BUILD],
    ),
    CommandSurface::leaf(command::CHANGELOG),
    CommandSurface::with_subcommands(
        command::AUTH,
        &[
            subcommand::STATUS,
            subcommand::LOGIN,
            subcommand::LOGOUT,
            subcommand::BROKER,
        ],
    ),
    CommandSurface::with_subcommands(command::LICENSE, &[subcommand::STATUS]),
    CommandSurface::leaf(command::DOCTOR),
    CommandSurface::leaf(command::ENV),
    CommandSurface::with_subcommands(
        command::DIAGNOSE,
        &[
            subcommand::PROXY,
            subcommand::ACCELERATOR,
            subcommand::UPDATE,
        ],
    ),
    CommandSurface::leaf(command::INSTALL),
    CommandSurface::leaf(command::INSTALL_MODULES),
    CommandSurface::leaf(command::MODULES),
    CommandSurface::leaf(command::RELEASES),
    CommandSurface::with_subcommands(
        command::PROJECTS,
        &[
            subcommand::UPGRADE,
            subcommand::CREATE,
            subcommand::CLONE,
            subcommand::LINK,
            subcommand::UNLINK,
            subcommand::INFO,
            subcommand::AUDIT,
            subcommand::ADD,
            subcommand::REMOVE,
            subcommand::PIN,
            subcommand::UNPIN,
            subcommand::CLOSE,
        ],
    ),
    CommandSurface::leaf(command::OPEN),
    CommandSurface::leaf(command::SELF_UPDATE),
    CommandSurface::with_argument(command::DOCS, &[subcommand::CACHE, subcommand::TOC]),
    CommandSurface::leaf(command::STATUS),
    CommandSurface::with_subcommands(
        command::PLUGIN,
        &[
            subcommand::INSTALL,
            subcommand::UPGRADE,
            subcommand::REMOVE,
            subcommand::LIST,
            subcommand::UNINSTALL,
        ],
    ),
    CommandSurface::with_subcommands(
        command::SKILL,
        &[
            subcommand::INSTALL,
            subcommand::REFRESH,
            subcommand::SHOW,
            subcommand::UNINSTALL,
        ],
    ),
    CommandSurface::leaf(command::SKILLS),
    CommandSurface::with_subcommands(
        command::TEMPLATES,
        &[subcommand::INFO, subcommand::CREATE, subcommand::PACK],
    ),
    CommandSurface::leaf(command::CLOUD),
    CommandSurface::leaf(command::BUILD),
    CommandSurface::leaf(command::CLOSE),
    CommandSurface::leaf(command::COMMAND),
    CommandSurface::leaf(command::UNINSTALL),
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
    flag::LIST,
    flag::LOCAL,
    flag::SKILL_NAME,
    flag::BRIEF,
    flag::TYPE,
    flag::NAME,
    flag::DISPLAY_NAME,
    flag::DESCRIPTION,
    flag::OUTPUT_DIR,
    flag::OUTPUT,
    flag::OVERWRITE,
    flag::TEMPLATE,
    flag::NO_CLOUD,
    flag::OPEN,
    flag::BARE,
    flag::VERSION,
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
    error_code::EDITOR_BLOCKED_BY_DIALOG,
    error_code::PROJECTS_CLOSE_BLOCKED_BY_DIALOG,
    error_code::INVALID_COMMAND_ARGS,
    error_code::EDITOR_NOT_INSTALLED,
    error_code::NOT_A_PROJECT,
    error_code::PROJECTS_REMOVE_NOT_FOUND,
    error_code::VCS_TOKEN_REQUIRED,
    error_code::GIT_START_FAILED,
    error_code::LOCAL_STORE_UNWRITABLE,
    error_code::LINK_VCS_DELETE_REMOTE_REQUIRES_YES,
    error_code::INSTALL_HEADLESS_UNAVAILABLE,
    error_code::BAD_ARGS,
];

/// Codes the app recognises but does not give a dedicated remediation.
pub const CLI_CODES: &[&str] = &[
    cli_code::COMMAND_FAILED,
    cli_code::NOT_FOUND,
    cli_code::STATUS_NO_INSTANCES,
    cli_code::DOWNLOAD_FAILED,
    cli_code::CHECKSUM_MISMATCH,
    cli_code::CANCELLED,
    cli_code::INTERRUPTED,
    cli_code::DOCS_NOT_FOUND,
    cli_code::EDITORS_PRUNE_EDITOR_CHECK_FAILED,
    cli_code::EDITORS_PRUNE_EDITOR_CHECK_UNAVAILABLE,
    cli_code::EDITOR_CONNECTION_LOST,
    cli_code::EDITOR_NOT_RUNNING,
    cli_code::EDITOR_NOT_READY,
    cli_code::UNSUPPORTED_STATE,
    cli_code::TEMPLATE_OTHER_VERSION_ON_DISK,
    cli_code::AUTH_BROKER_UNAVAILABLE,
    cli_code::AUTH_BROKER_REFUSED,
    cli_code::CREDENTIAL_STORE_ACCESS_DENIED,
];

/// One documented exit code and everything the app derives from it.
///
/// Before this table existed, the code name, message, retry flag, remediation
/// fallback and mapped set each lived in a separate `match`, so adding a code
/// touched four places that could drift. Everything now reads this one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCodeSpec {
    pub code: i32,
    /// Stable name used when the CLI gives no envelope code.
    pub name: &'static str,
    /// Human summary used when the CLI gives no message.
    pub message: &'static str,
    /// Whether a read at this exit code is safe to retry.
    pub retryable: bool,
    /// The stable error code whose remediation this exit code falls back to.
    /// `None` keeps the generic rendering.
    pub remediation_code: Option<&'static str>,
}

/// Every documented exit code, in numeric order.
pub const EXIT_CODES: &[ExitCodeSpec] = &[
    ExitCodeSpec {
        code: 0,
        name: "SUCCESS",
        message: "The Unity CLI command succeeded.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 1,
        name: "ERROR",
        message: "The Unity CLI command failed.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 2,
        name: "BAD_ARGS",
        message: "The Unity CLI rejected the arguments.",
        retryable: false,
        remediation_code: Some(error_code::BAD_ARGS),
    },
    ExitCodeSpec {
        code: 3,
        name: "AUTH_ERROR",
        message: "Unity sign-in is required, or the session expired.",
        retryable: false,
        remediation_code: Some(error_code::AUTH_ERROR),
    },
    ExitCodeSpec {
        code: 4,
        name: "NEEDS_CONFIG",
        message: "The Unity CLI needs configuration before this can run.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 6,
        name: "COMMAND_FAILED",
        message: "The Unity CLI command failed.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 7,
        name: "NETWORK_UNREACHABLE",
        message: "A Unity service could not be reached.",
        retryable: true,
        remediation_code: Some(error_code::NETWORK_UNREACHABLE),
    },
    ExitCodeSpec {
        code: 8,
        name: "TESTS_FAILED",
        message: "The tests ran, and some failed.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 9,
        name: "INSTALL_LOCKED",
        message: "Another Unity install is holding the install lock.",
        retryable: true,
        remediation_code: Some(error_code::INSTALL_LOCKED),
    },
    ExitCodeSpec {
        code: 10,
        name: "READY_TIMEOUT",
        message: "The Unity Editor did not become ready in time.",
        retryable: true,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 11,
        name: "NOT_FOUND",
        message: "The requested project, release, or module does not exist.",
        retryable: false,
        // Deliberately unmapped: 1.1.0 moved several different not-found
        // failures onto 11, so the code must name the target.
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 12,
        name: "EDITOR_UNREACHABLE",
        message: "No running Unity Editor answered.",
        retryable: true,
        remediation_code: Some(error_code::EDITOR_UNREACHABLE),
    },
    ExitCodeSpec {
        code: 13,
        name: "REMOTE_COMMAND_FAILED",
        message: "The connected Editor does not expose that command.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 14,
        name: "TIMEOUT",
        message: "The operation timed out.",
        retryable: true,
        remediation_code: Some(error_code::TIMEOUT),
    },
    ExitCodeSpec {
        code: 15,
        name: "RUN_FAILED",
        message: "A build or job finished unsuccessfully.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 16,
        name: "JOB_SUSPENDED",
        message: "A job is waiting for a person to resume or approve it.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 17,
        name: "UNAVAILABLE",
        message: "The target is busy; wait for it to finish, then retry.",
        retryable: true,
        remediation_code: Some(error_code::UNAVAILABLE),
    },
    ExitCodeSpec {
        code: 18,
        name: "AWAITING_INPUT",
        message: "The target is waiting on a person to answer a dialog or prompt.",
        retryable: true,
        remediation_code: Some(error_code::AWAITING_INPUT),
    },
    ExitCodeSpec {
        code: 130,
        name: "INTERRUPTED",
        message: "The operation was interrupted.",
        retryable: true,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 141,
        name: "OUTPUT_CLOSED",
        message: "The output reader closed the pipe.",
        retryable: false,
        remediation_code: None,
    },
    ExitCodeSpec {
        code: 143,
        name: "TERMINATED",
        message: "The operation was terminated.",
        retryable: true,
        remediation_code: None,
    },
];

pub fn exit_code_spec(code: i32) -> Option<&'static ExitCodeSpec> {
    EXIT_CODES.iter().find(|spec| spec.code == code)
}

pub fn is_command(token: &str) -> bool {
    COMMANDS.iter().any(|command| command.name == token)
}

pub fn is_flag(token: &str) -> bool {
    FLAGS.contains(&token)
}

/// Flags every invocation carries. A changelog entry that names a subtree the
/// app does not drive still affects the app when it changes one of these.
pub fn is_global_flag(token: &str) -> bool {
    [
        flag::FORMAT,
        flag::NO_PAGER,
        flag::NO_BANNER,
        flag::NON_INTERACTIVE,
    ]
    .contains(&token)
}

pub fn is_error_code(token: &str) -> bool {
    ERROR_CODES.contains(&token) || CLI_CODES.contains(&token)
}

/// Whether the app has an explicit remedy for this exit code.
pub fn is_mapped_exit_code(code: i32) -> bool {
    exit_code_spec(code).is_some_and(|spec| spec.remediation_code.is_some())
}

/// Resolves `unity <command> [<subcommand>]` (or a bare command span) to the
/// command path the CLI docs index uses. A leading `unity` is optional, so a
/// backticked `` `install-modules --architecture` `` is recognised too.
///
/// An unknown command returns `None`. A second token that is not a registered
/// subcommand returns the bare command only when the command takes a free
/// argument (a docs topic) or has no subcommands at all (a version/path
/// argument); otherwise it is an unknown subtree such as `pipeline automation`
/// and returns `None`. Flags never become part of the path.
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
        if second.starts_with('-') {
            return Some(path);
        }
        if command.subcommands.contains(second) {
            path.push(' ');
            path.push_str(second);
            return Some(path);
        }
        if command.subcommands.is_empty() || command.free_argument {
            return Some(path);
        }
        return None;
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
        assert_eq!(
            command_path(&["unity", "skill", "install", "cursor"]).as_deref(),
            Some("skill install")
        );
        assert_eq!(
            command_path(&["unity", "skills"]).as_deref(),
            Some("skills")
        );
        for flag in [flag::LIST, flag::LOCAL, flag::SKILL_NAME] {
            assert!(is_flag(flag), "{flag} must be in the surface registry");
        }
    }

    #[test]
    fn unknown_commands_are_not_surface() {
        assert!(command_path(&["unity", "definitely-not-a-command"]).is_none());
        assert!(!is_command("definitely-not-a-command"));
        assert!(!is_flag("--quiet"));
        assert!(!is_error_code("SOMETHING_ELSE"));
    }

    #[test]
    fn unknown_subtrees_are_not_app_surface() {
        // Pipeline Automation is a separate subtree from the Pipeline
        // Editors/Cloud Build surface this app drives.
        assert!(command_path(&["unity", "pipeline", "automation"]).is_none());
        assert!(command_path(&["unity", "projects", "clean"]).is_none());
    }

    #[test]
    fn argument_commands_keep_the_bare_path() {
        assert_eq!(
            command_path(&["unity", "docs", "MonoBehaviour.Start"]).as_deref(),
            Some("docs")
        );
        assert_eq!(
            command_path(&["unity", "install", "6000.6.6f1"]).as_deref(),
            Some("install")
        );
    }

    #[test]
    fn newly_registered_paths_are_recognised() {
        assert_eq!(
            command_path(&["unity", "projects", "close"]).as_deref(),
            Some("projects close")
        );
        assert_eq!(command_path(&["unity", "close"]).as_deref(), Some("close"));
        assert_eq!(
            command_path(&["unity", "uninstall", "6000.6.6f1"]).as_deref(),
            Some("uninstall")
        );
        assert_eq!(
            command_path(&["unity", "skill", "show", "--skill", "unity-cli"]).as_deref(),
            Some("skill show")
        );
        assert_eq!(
            command_path(&["unity", "skill", "uninstall", "unity-cli"]).as_deref(),
            Some("skill uninstall")
        );
        assert_eq!(
            command_path(&["unity", "templates", "info", "com.unity.x"]).as_deref(),
            Some("templates info")
        );
        assert_eq!(
            command_path(&["unity", "command", "editor", "status"]).as_deref(),
            Some("command")
        );
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
        for spec in EXIT_CODES
            .iter()
            .filter(|spec| spec.remediation_code.is_some())
        {
            let error =
                crate::errors::UnityError::new("ERROR", "test").with_exit_code(Some(spec.code));
            assert!(
                crate::errors::remediation_for(&error, None).is_some(),
                "exit code {} is mapped but has no remediation",
                spec.code
            );
        }
    }

    #[test]
    fn every_exit_code_has_exactly_one_spec() {
        let mut seen = std::collections::BTreeSet::new();
        for spec in EXIT_CODES {
            assert!(
                seen.insert(spec.code),
                "duplicate spec for exit {}",
                spec.code
            );
            assert_eq!(
                crate::errors::code_for_exit_code(spec.code),
                spec.name,
                "exit {} name drifted",
                spec.code
            );
            assert_eq!(
                crate::errors::message_for_exit_code(spec.code),
                spec.message,
                "exit {} message drifted",
                spec.code
            );
            assert_eq!(
                crate::errors::is_retryable_exit_code(spec.code),
                spec.retryable,
                "exit {} retryable flag drifted",
                spec.code
            );
            assert_eq!(
                is_mapped_exit_code(spec.code),
                spec.remediation_code.is_some(),
                "exit {} mapped flag drifted",
                spec.code
            );
        }

        for code in [
            0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 130, 141, 143,
        ] {
            assert!(seen.contains(&code), "documented exit {code} is missing");
        }
    }

    #[test]
    fn unknown_exit_codes_stay_generic() {
        assert_eq!(crate::errors::code_for_exit_code(99), "ERROR");
        assert_eq!(
            crate::errors::message_for_exit_code(99),
            "The Unity CLI command failed."
        );
        assert!(!crate::errors::is_retryable_exit_code(99));
        assert!(!is_mapped_exit_code(99));
    }

    #[test]
    fn recognises_codes_without_a_dedicated_remediation() {
        for code in CLI_CODES {
            assert!(is_error_code(code), "{code} should be recognised");
        }
        assert!(is_error_code(cli_code::EDITOR_CONNECTION_LOST));
        assert!(!is_error_code("SOMETHING_ELSE"));
    }

    #[test]
    fn required_command_paths_resolve_through_the_registry() {
        for path in REQUIRED_COMMAND_PATHS {
            let tokens: Vec<&str> = path.split_whitespace().collect();
            let resolved =
                command_path(&tokens).unwrap_or_else(|| panic!("{path} did not resolve"));
            assert!(path.starts_with(&resolved), "{path} resolved to {resolved}");
            assert!(
                is_command(tokens[0]),
                "{path} starts with an unregistered command"
            );
        }
    }
}
