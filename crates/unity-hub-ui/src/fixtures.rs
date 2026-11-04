//! Deterministic view models for the design gallery and UI tests.
//!
//! These fixtures never touch the CLI or the controller. They are the contract
//! surface a designer can review without a Unity installation.

use unity_hub_model::{
    AccountCardVm, AuthVm, BadgeVm, BreakingChangeVm, ChangelogDataVm, ChangelogSectionVm,
    ChangelogVm, CliCardVm, CliConnectionVm, CliDetailsVm, EditorModulesVm, EditorRowVm, EditorsVm,
    InstallCliVm, InstallEditorVm, JobLogVm, JobPhase, JobVm, LicenseRowVm, LicenseVm, LoadState,
    ModalVm, ModuleOptionVm, OverviewVm, Page, PageVm, ProjectRowVm, ProjectsVm, ReleaseRowVm,
    RunningEditorVm, SectionKind, SettingsVm, ShellVm, TextScaleOptionVm, Tone, UiError, UpdateVm,
    UpgradeProjectVm, ViewModel,
};

pub struct Fixture {
    pub name: &'static str,
    pub view_model: ViewModel,
}

pub fn all() -> Vec<Fixture> {
    vec![
        Fixture {
            name: "Overview",
            view_model: overview(),
        },
        Fixture {
            name: "Overview · CLI missing",
            view_model: overview_missing(),
        },
        Fixture {
            name: "Overview · load error",
            view_model: overview_error(),
        },
        Fixture {
            name: "Editors",
            view_model: editors(),
        },
        Fixture {
            name: "Projects",
            view_model: projects(),
        },
        Fixture {
            name: "Changelog",
            view_model: changelog(),
        },
        Fixture {
            name: "Settings",
            view_model: settings(),
        },
        Fixture {
            name: "Modal · install CLI",
            view_model: install_cli_modal(),
        },
        Fixture {
            name: "Modal · Editor modules",
            view_model: editor_modules_modal(),
        },
        Fixture {
            name: "Modal · project upgrade",
            view_model: upgrade_modal(),
        },
        Fixture {
            name: "Job · running",
            view_model: job_running(),
        },
        Fixture {
            name: "Job · failed",
            view_model: job_failed(),
        },
    ]
}

fn shell(page: Page) -> ShellVm {
    ShellVm {
        page,
        cli: CliConnectionVm::Connected {
            version: Some("1.0.0-beta.16".to_string()),
        },
        notice: None,
        job: None,
    }
}

fn base(page: Page, page_vm: PageVm) -> ViewModel {
    ViewModel {
        shell: shell(page),
        page: page_vm,
        modal: None,
        text_scale: 1.0,
    }
}

fn overview() -> ViewModel {
    base(
        Page::Overview,
        PageVm::Overview(OverviewVm {
            cli: CliCardVm::Ready {
                details: CliDetailsVm {
                    version: "1.0.0-beta.16".to_string(),
                    channel: "beta".to_string(),
                    platform: "linux x86_64".to_string(),
                    runtime: "rust 1.99".to_string(),
                    executable: "/home/ada/.local/bin/unity".to_string(),
                    resolved_via: "PATH".to_string(),
                    install_path: "/home/ada/Unity/Editors".to_string(),
                    build_date: "Oct 24, 2026".to_string(),
                },
                error: None,
            },
            update: LoadState::Ready(UpdateVm {
                available: false,
                current_version: "1.0.0-beta.16".to_string(),
                latest_version: "1.0.0-beta.16".to_string(),
            }),
            account: AccountCardVm {
                auth: AuthVm::SignedIn {
                    name: "Ada Lovelace".to_string(),
                    email: "ada@example.com".to_string(),
                },
                license: LicenseVm::Ready {
                    active: true,
                    licenses: vec![LicenseRowVm {
                        product: "Unity Pro".to_string(),
                        kind: "subscription".to_string(),
                    }],
                },
            },
            running: LoadState::Ready(vec![
                RunningEditorVm {
                    project_path: "/home/ada/Games/Orbital".to_string(),
                    editor_version: "6000.1.1f1".to_string(),
                    pid: "4242".to_string(),
                    phase: Some("ready".to_string()),
                    pipeline_version: Some("0.9.1-exp.1".to_string()),
                    update_available: true,
                    safe_mode: false,
                    dialog: None,
                },
                RunningEditorVm {
                    project_path: "/home/ada/Tools/LevelKit".to_string(),
                    editor_version: "2022.3.62f1".to_string(),
                    pid: "5151".to_string(),
                    phase: Some("safe mode".to_string()),
                    pipeline_version: None,
                    update_available: false,
                    safe_mode: true,
                    dialog: Some("Save changes?".to_string()),
                },
            ]),
        }),
    )
}

fn overview_missing() -> ViewModel {
    let mut vm = base(
        Page::Overview,
        PageVm::Overview(OverviewVm {
            cli: CliCardVm::Missing,
            update: LoadState::Idle,
            account: AccountCardVm {
                auth: AuthVm::SignedOut { expired: false },
                license: LicenseVm::Idle,
            },
            running: LoadState::Ready(Vec::new()),
        }),
    );
    vm.shell.cli = CliConnectionVm::Missing;
    vm
}

fn overview_error() -> ViewModel {
    base(
        Page::Overview,
        PageVm::Overview(OverviewVm {
            cli: CliCardVm::Failed(UiError {
                message: "Could not resolve the Unity CLI.".to_string(),
                detail: Some("Checked PATH and the Hub install receipt.".to_string()),
            }),
            update: LoadState::Failed {
                error: UiError {
                    message: "Update check failed.".to_string(),
                    detail: None,
                },
                previous: None,
            },
            account: AccountCardVm {
                auth: AuthVm::Failed(UiError {
                    message: "Not signed in.".to_string(),
                    detail: None,
                }),
                license: LicenseVm::Failed(UiError {
                    message: "License service unavailable.".to_string(),
                    detail: None,
                }),
            },
            running: LoadState::Loading { previous: None },
        }),
    )
}

fn editors() -> ViewModel {
    base(
        Page::Editors,
        PageVm::Editors(EditorsVm {
            installed: LoadState::Ready(vec![
                EditorRowVm {
                    version: "6000.1.1f1".to_string(),
                    architecture: "x86_64".to_string(),
                    is_default: true,
                    modules: "linux-il2cpp, linux-server".to_string(),
                },
                EditorRowVm {
                    version: "2022.3.62f1".to_string(),
                    architecture: "x86_64".to_string(),
                    is_default: false,
                    modules: "No modules reported".to_string(),
                },
            ]),
            install: InstallEditorVm {
                version_input: "lts".to_string(),
                modules: LoadState::Ready(vec![
                    ModuleOptionVm {
                        id: "linux-il2cpp".to_string(),
                        label: "Linux IL2CPP Build Support  ·  1.2 GB".to_string(),
                        selected: true,
                    },
                    ModuleOptionVm {
                        id: "webgl".to_string(),
                        label: "WebGL Build Support  ·  800 MB".to_string(),
                        selected: false,
                    },
                ]),
                available_count: 2,
                accept_eula: false,
                can_install: true,
                preview: None,
                preview_note: None,
                preview_loading: false,
            },
            releases: LoadState::Ready(vec![
                ReleaseRowVm {
                    version: "6000.3.0f1".to_string(),
                    stream: "LTS".to_string(),
                    stream_tone: Tone::Success,
                    installed: false,
                },
                ReleaseRowVm {
                    version: "6000.2.0b1".to_string(),
                    stream: "Beta".to_string(),
                    stream_tone: Tone::Warning,
                    installed: true,
                },
            ]),
        }),
    )
}

fn projects() -> ViewModel {
    base(
        Page::Projects,
        PageVm::Projects(ProjectsVm {
            search: String::new(),
            total_count: 2,
            visible_count: 2,
            projects: LoadState::Ready(vec![
                ProjectRowVm {
                    path: "/home/ada/Games/Orbital".to_string(),
                    title: "Orbital".to_string(),
                    version: "6000.1.1f1".to_string(),
                    pipeline: Some(BadgeVm {
                        label: "URP".to_string(),
                        tone: Tone::Highlight,
                    }),
                    build_target: "Linux".to_string(),
                    short_path: "…/Games/Orbital".to_string(),
                    modified: "Oct 31, 2026".to_string(),
                    badges: vec![
                        BadgeVm {
                            label: "Unity Cloud".to_string(),
                            tone: Tone::Info,
                        },
                        BadgeVm {
                            label: "favorite".to_string(),
                            tone: Tone::Accent,
                        },
                    ],
                },
                ProjectRowVm {
                    path: "/home/ada/Tools/LevelKit".to_string(),
                    title: "LevelKit".to_string(),
                    version: "2022.3.62f1".to_string(),
                    pipeline: None,
                    build_target: "—".to_string(),
                    short_path: "…/Tools/LevelKit".to_string(),
                    modified: "Sep 12, 2026".to_string(),
                    badges: vec![BadgeVm {
                        label: "Editor not installed".to_string(),
                        tone: Tone::Warning,
                    }],
                },
            ]),
        }),
    )
}

fn changelog() -> ViewModel {
    base(
        Page::Changelog,
        PageVm::Changelog(ChangelogVm {
            loaded: true,
            versions: vec!["1.0.0-beta.15".to_string(), "1.0.0-beta.16".to_string()],
            selected_target: Some("1.0.0-beta.16".to_string()),
            show_raw: false,
            state: LoadState::Ready(ChangelogDataVm {
                version: "1.0.0-beta.16".to_string(),
                source: "unity-changelog".to_string(),
                source_detail: "Unity CLI 1.0.0-beta.16 release notes".to_string(),
                entry_count: 12,
                breaking_changes: vec![BreakingChangeVm {
                    text: "The list subcommands were removed.".to_string(),
                    section: "Removed".to_string(),
                    reason: "removed".to_string(),
                }],
                sections: vec![
                    ChangelogSectionVm {
                        title: "Added".to_string(),
                        kind: SectionKind::Added,
                        entries: vec![
                            "`unity projects open` now reports the launched Editor.".to_string(),
                            "NDJSON progress includes module download totals.".to_string(),
                        ],
                    },
                    ChangelogSectionVm {
                        title: "Changed".to_string(),
                        kind: SectionKind::Changed,
                        entries: vec!["`--non-interactive` is now the default for machine output."
                            .to_string()],
                    },
                ],
            }),
            raw: LoadState::Idle,
        }),
    )
}

fn settings() -> ViewModel {
    base(
        Page::Settings,
        PageVm::Settings(SettingsVm {
            resolved_executable: "/home/ada/.local/bin/unity".to_string(),
            resolved_via: "PATH".to_string(),
            cli_path_input: String::new(),
            config_dir: "/home/ada/.config/unity-rust-hub".to_string(),
            settings_file: "/home/ada/.config/unity-rust-hub/settings.json".to_string(),
            text_scale_options: vec![
                TextScaleOptionVm {
                    percent: 100,
                    label: "100%".to_string(),
                },
                TextScaleOptionVm {
                    percent: 110,
                    label: "110%".to_string(),
                },
                TextScaleOptionVm {
                    percent: 125,
                    label: "125%".to_string(),
                },
                TextScaleOptionVm {
                    percent: 150,
                    label: "150%".to_string(),
                },
            ],
            text_scale_percent: 100,
            engine_info: "1.0.0-beta.16 · beta · linux/x86_64".to_string(),
        }),
    )
}

fn install_cli_modal() -> ViewModel {
    let mut vm = overview_missing();
    vm.modal = Some(ModalVm::InstallCli(InstallCliVm {
        channels: vec![
            "stable".to_string(),
            "beta".to_string(),
            "alpha".to_string(),
        ],
        selected_channel: "stable".to_string(),
        consent: false,
        command_preview: "curl -fsSL https://unity.com/install.sh | bash".to_string(),
        can_install: false,
    }));
    vm
}

fn editor_modules_modal() -> ViewModel {
    let mut vm = editors();
    vm.modal = Some(ModalVm::EditorModules(EditorModulesVm {
        editor_version: "6000.1.1f1".to_string(),
        installed_modules: vec!["linux-il2cpp".to_string(), "linux-server".to_string()],
        modules: LoadState::Ready(vec![
            ModuleOptionVm {
                id: "webgl".to_string(),
                label: "WebGL Build Support  ·  800 MB".to_string(),
                selected: true,
            },
            ModuleOptionVm {
                id: "android".to_string(),
                label: "Android Build Support  ·  1.4 GB".to_string(),
                selected: false,
            },
        ]),
        selected_count: 1,
        accept_eula: true,
        can_install: true,
    }));
    vm
}

fn upgrade_modal() -> ViewModel {
    let mut vm = projects();
    vm.modal = Some(ModalVm::UpgradeProject(UpgradeProjectVm {
        project_title: "Orbital".to_string(),
        project_path_short: "…/Games/Orbital".to_string(),
        current_version: "6000.1.1f1".to_string(),
        target_options: vec!["6000.3.0f1".to_string()],
        selected_target: Some("6000.3.0f1".to_string()),
        confirmed: false,
        can_upgrade: false,
    }));
    vm
}

fn job_running() -> ViewModel {
    let mut vm = overview();
    vm.shell.job = Some(JobVm {
        kind: "editor-install".to_string(),
        command: "unity install lts --yes --format ndjson".to_string(),
        phase: JobPhase::Running,
        progress: 42.0,
        message: Some("Downloading modules…".to_string()),
        error: None,
        logs: vec![JobLogVm {
            is_stderr: false,
            text: "downloaded 4.2 GB of 9.8 GB".to_string(),
        }],
    });
    vm
}

fn job_failed() -> ViewModel {
    let mut vm = overview();
    vm.shell.notice = Some(unity_hub_model::NoticeVm {
        tone: Tone::Danger,
        text: "The job failed.".to_string(),
    });
    vm.shell.job = Some(JobVm {
        kind: "project-upgrade".to_string(),
        command: "unity projects upgrade /games/orbital --to 6000.3.0f1 --yes".to_string(),
        phase: JobPhase::Failed,
        progress: 0.0,
        message: Some("Upgrading project…".to_string()),
        error: Some(UiError {
            message: "Project upgrade failed.".to_string(),
            detail: Some("The target Editor is not installed.".to_string()),
        }),
        logs: vec![JobLogVm {
            is_stderr: true,
            text: "Editor 6000.3.0f1 is not installed.".to_string(),
        }],
    });
    vm
}
