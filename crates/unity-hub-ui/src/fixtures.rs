//! Deterministic view models for the design gallery and UI tests.
//!
//! These fixtures never touch the CLI or the controller. They are the contract
//! surface a designer can review without a Unity installation.

use unity_hub_model::{
    AccountCardVm, AuthVm, BadgeVm, BreakingChangeVm, ChangelogDataVm, ChangelogSectionVm,
    ChangelogVm, CliCardVm, CliConnectionVm, CliDetailsVm, EditorModulesVm, EditorRowVm, EditorsVm,
    InstallCliVm, InstallEditorVm, JobPhase, JobVm, LicenseRowVm, LicenseVm, LoadState, ModalVm,
    ModuleOptionVm, NewProjectEditorVm, NewProjectPreflightState, NewProjectPreflightVm,
    NewProjectResultVm, NewProjectStep, NewProjectVm, OverviewVm, Page, PageVm, ProcessMetricsVm,
    ProcessRowVm, ProcessesVm, ProjectRowVm, ProjectsVm, ReleaseRowVm, RemoveProjectVm,
    RunningEditorVm, SectionKind, SettingsVm, ShellVm, TemplateAuthoringMode, TemplateAuthoringVm,
    TemplateDetailVm, TemplateFormVm, TemplateKindFilter, TemplateRowVm, TemplatesVm,
    TextScaleOptionVm, Tone, UiError, UpdateVm, UpgradeProjectVm, ViewModel,
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
            name: "Templates",
            view_model: templates(),
        },
        Fixture {
            name: "Templates · empty",
            view_model: templates_empty(),
        },
        Fixture {
            name: "Templates · error",
            view_model: templates_error(),
        },
        Fixture {
            name: "Processes",
            view_model: processes(),
        },
        Fixture {
            name: "Processes · empty",
            view_model: processes_empty(),
        },
        Fixture {
            name: "Processes · unreachable",
            view_model: processes_unreachable(),
        },
        Fixture {
            name: "Processes · metrics unknown",
            view_model: processes_metrics_unknown(),
        },
        Fixture {
            name: "New project · blocked",
            view_model: new_project_blocked(),
        },
        Fixture {
            name: "New project · details",
            view_model: new_project(NewProjectStep::Details),
        },
        Fixture {
            name: "New project · editor",
            view_model: new_project(NewProjectStep::Editor),
        },
        Fixture {
            name: "New project · template",
            view_model: new_project(NewProjectStep::Template),
        },
        Fixture {
            name: "New project · review",
            view_model: new_project(NewProjectStep::Review),
        },
        Fixture {
            name: "New project · result",
            view_model: new_project(NewProjectStep::Result),
        },
        Fixture {
            name: "New project · partial",
            view_model: new_project_partial(),
        },
        Fixture {
            name: "Modal · template create",
            view_model: template_authoring(TemplateAuthoringMode::Create, false),
        },
        Fixture {
            name: "Modal · template pack",
            view_model: template_authoring(TemplateAuthoringMode::Pack, true),
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
            name: "Modal · remove project",
            view_model: remove_modal(),
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
            version: Some("1.0.0".to_string()),
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
                    version: "1.0.0".to_string(),
                    channel: "stable".to_string(),
                    platform: "linux x86_64".to_string(),
                    runtime: "rust 1.99".to_string(),
                    executable: "/home/ada/.local/bin/unity".to_string(),
                    resolved_via: "PATH".to_string(),
                    install_path: "/home/ada/Unity/Editors".to_string(),
                    build_date: "Nov 13, 2026".to_string(),
                },
                error: None,
            },
            update: LoadState::Ready(UpdateVm {
                available: false,
                current_version: "1.0.0".to_string(),
                latest_version: "1.0.0".to_string(),
            }),
            account: AccountCardVm {
                auth: AuthVm::SignedIn {
                    name: "Ada Lovelace".to_string(),
                    email: "ada@example.com".to_string(),
                    stale: false,
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
            editor_count: Some(2),
            project_count: Some(2),
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
            editor_count: None,
            project_count: None,
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
            editor_count: None,
            project_count: None,
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
                    modules: vec!["linux-il2cpp".to_string(), "linux-server".to_string()],
                },
                EditorRowVm {
                    version: "2022.3.62f1".to_string(),
                    architecture: "x86_64".to_string(),
                    is_default: false,
                    modules: Vec::new(),
                },
                EditorRowVm {
                    version: "6000.3.15f1".to_string(),
                    architecture: "x86_64".to_string(),
                    is_default: false,
                    modules: vec![
                        "Android Build Support".to_string(),
                        "Android SDK & NDK Tools".to_string(),
                        "Documentation".to_string(),
                    ],
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
            total_count: 3,
            visible_count: 3,
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
                    is_favorite: true,
                    badges: vec![BadgeVm {
                        label: "Unity Cloud".to_string(),
                        tone: Tone::Info,
                    }],
                },
                ProjectRowVm {
                    path: "/home/ada/Tools/LevelKit".to_string(),
                    title: "LevelKit".to_string(),
                    version: "2022.3.62f1".to_string(),
                    pipeline: None,
                    build_target: "—".to_string(),
                    short_path: "…/Tools/LevelKit".to_string(),
                    modified: "Sep 12, 2026".to_string(),
                    is_favorite: false,
                    badges: vec![BadgeVm {
                        label: "Editor not installed".to_string(),
                        tone: Tone::Warning,
                    }],
                },
                ProjectRowVm {
                    path: "C:\\Users\\ada\\DustGames\\GitProjects\\RoomBattleMultiplayer"
                        .to_string(),
                    title: "RoomBattleMultiplayer".to_string(),
                    version: "6000.0.61f1".to_string(),
                    pipeline: Some(BadgeVm {
                        label: "URP".to_string(),
                        tone: Tone::Highlight,
                    }),
                    build_target: "Windows".to_string(),
                    short_path: "…\\RoomBattleMultiplayer".to_string(),
                    modified: "Dec 4, 2026".to_string(),
                    is_favorite: false,
                    badges: vec![
                        BadgeVm {
                            label: "Unity Cloud".to_string(),
                            tone: Tone::Info,
                        },
                        BadgeVm {
                            label: "github".to_string(),
                            tone: Tone::Neutral,
                        },
                    ],
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
            versions: vec!["1.0.0-beta.15".to_string(), "1.0.0".to_string()],
            selected_target: Some("1.0.0".to_string()),
            show_raw: false,
            state: LoadState::Ready(ChangelogDataVm {
                version: "1.0.0".to_string(),
                source: "unity-changelog".to_string(),
                source_detail: "Unity CLI 1.0.0 release notes".to_string(),
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
            font_options: vec![
                "Consolas".to_string(),
                "Segoe UI".to_string(),
                "Tahoma".to_string(),
            ]
            .into(),
            font_family: "Tahoma".to_string(),
            engine_info: "1.0.0 · stable · linux/x86_64".to_string(),
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

fn remove_modal() -> ViewModel {
    let mut vm = projects();
    vm.modal = Some(ModalVm::RemoveProject(RemoveProjectVm {
        project_title: "Orbital".to_string(),
        project_path_short: "…/Games/Orbital".to_string(),
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
        logs: "downloaded 4.2 GB of 9.8 GB\n".into(),
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
        logs: "! Editor 6000.3.0f1 is not installed.\n".into(),
    });
    vm
}

fn template_rows() -> Vec<TemplateRowVm> {
    vec![
        TemplateRowVm {
            name: "com.unity.template.urp-blank".to_string(),
            display_name: "Universal 3D".to_string(),
            kind: "CORE".to_string(),
            version: "17.1.0".to_string(),
            status: "ready".to_string(),
            status_tone: Tone::Success,
            installed: true,
            pipeline: Some("URP".to_string()),
        },
        TemplateRowVm {
            name: "com.unity.template.universal-2d".to_string(),
            display_name: "Universal 2D".to_string(),
            kind: "CORE".to_string(),
            version: "6.1.2".to_string(),
            status: "ready".to_string(),
            status_tone: Tone::Success,
            installed: true,
            pipeline: Some("URP · 2D".to_string()),
        },
        TemplateRowVm {
            name: "com.unity.template.hdrp-blank".to_string(),
            display_name: "High Definition 3D".to_string(),
            kind: "CORE".to_string(),
            version: "17.1.1".to_string(),
            status: "upgradable".to_string(),
            status_tone: Tone::Warning,
            installed: true,
            pipeline: Some("HDRP".to_string()),
        },
        TemplateRowVm {
            name: "com.unity.template.mixed-reality".to_string(),
            display_name: "Mixed Reality (MR)".to_string(),
            kind: "CORE".to_string(),
            version: "3.0.0".to_string(),
            status: "downloadable".to_string(),
            status_tone: Tone::Info,
            installed: false,
            pipeline: None,
        },
    ]
}

fn template_detail() -> TemplateDetailVm {
    TemplateDetailVm {
        name: "com.unity.template.urp-blank".to_string(),
        display_name: "Universal 3D".to_string(),
        description: "The URP blank template includes the settings and assets you need to start creating with the Universal Render Pipeline.".to_string(),
        kind: "CORE".to_string(),
        version: "17.1.0".to_string(),
        status: "READY".to_string(),
        installed: true,
        render_pipeline: Some("URP".to_string()),
        packages: vec![
            "com.unity.render-pipelines.universal 17.0.4".to_string(),
            "com.unity.ugui 2.0.0".to_string(),
            "com.unity.inputsystem 1.14.2".to_string(),
        ],
        previous_versions: vec!["17.0.9".to_string(), "17.0.8".to_string()],
    }
}

fn templates() -> ViewModel {
    base(
        Page::Templates,
        PageVm::Templates(TemplatesVm {
            editors: LoadState::Ready(vec!["6000.3.15f1".to_string(), "6000.6.6f1".to_string()]),
            selected_editor: Some("6000.3.15f1".to_string()),
            filter: TemplateKindFilter::All,
            installed_only: false,
            list: LoadState::Ready(template_rows()),
            detail: Some(template_detail()),
            detail_loading: false,
            detail_error: None,
        }),
    )
}

fn templates_empty() -> ViewModel {
    base(
        Page::Templates,
        PageVm::Templates(TemplatesVm {
            editors: LoadState::Ready(vec!["6000.3.15f1".to_string()]),
            selected_editor: Some("6000.3.15f1".to_string()),
            filter: TemplateKindFilter::Custom,
            installed_only: true,
            list: LoadState::Ready(Vec::new()),
            detail: None,
            detail_loading: false,
            detail_error: None,
        }),
    )
}

fn templates_error() -> ViewModel {
    base(
        Page::Templates,
        PageVm::Templates(TemplatesVm {
            editors: LoadState::Ready(vec!["6000.3.15f1".to_string()]),
            selected_editor: Some("6000.3.15f1".to_string()),
            filter: TemplateKindFilter::All,
            installed_only: false,
            list: LoadState::Failed {
                error: UiError {
                    message: "Could not read templates.".to_string(),
                    detail: Some("The CLI returned an unexpected document.".to_string()),
                },
                previous: None,
            },
            detail: None,
            detail_loading: false,
            detail_error: None,
        }),
    )
}

fn process_rows() -> Vec<ProcessRowVm> {
    vec![
        ProcessRowVm {
            project_path: "/home/ada/Games/Orbital".to_string(),
            project_name: "Orbital".to_string(),
            editor_version: "6000.3.15f1".to_string(),
            pid: Some("4242".to_string()),
            port: Some("7800".to_string()),
            state: "ready".to_string(),
            state_tone: Tone::Success,
            phase: Some("play mode".to_string()),
            play_mode: true,
            frame_count: Some("18420".to_string()),
            pipeline_version: Some("0.9.1-exp.1".to_string()),
            safe_mode: false,
            dialog: None,
            metrics: Some(ProcessMetricsVm {
                cpu: "12.4%".to_string(),
                memory: "1.8 GB".to_string(),
                uptime: "2h 14m".to_string(),
            }),
        },
        ProcessRowVm {
            project_path: "/home/ada/Tools/LevelKit".to_string(),
            project_name: "LevelKit".to_string(),
            editor_version: "2022.3.62f1".to_string(),
            pid: Some("5151".to_string()),
            port: None,
            state: "starting".to_string(),
            state_tone: Tone::Info,
            phase: Some("loading project".to_string()),
            play_mode: false,
            frame_count: Some("12".to_string()),
            pipeline_version: None,
            safe_mode: true,
            dialog: Some("Save changes?".to_string()),
            metrics: Some(ProcessMetricsVm {
                cpu: "—".to_string(),
                memory: "640 MB".to_string(),
                uptime: "18s".to_string(),
            }),
        },
    ]
}

fn processes() -> ViewModel {
    base(
        Page::Processes,
        PageVm::Processes(ProcessesVm {
            rows: LoadState::Ready(process_rows()),
            metrics_enabled: true,
            pipeline_available: true,
            last_refreshed: Some("14:32:08".to_string()),
        }),
    )
}

fn processes_empty() -> ViewModel {
    base(
        Page::Processes,
        PageVm::Processes(ProcessesVm {
            rows: LoadState::Ready(Vec::new()),
            metrics_enabled: false,
            pipeline_available: false,
            last_refreshed: Some("14:32:08".to_string()),
        }),
    )
}

fn processes_unreachable() -> ViewModel {
    let mut rows = process_rows();
    rows[1].state = "unreachable".to_string();
    rows[1].state_tone = Tone::Danger;
    rows[1].phase = None;
    base(
        Page::Processes,
        PageVm::Processes(ProcessesVm {
            rows: LoadState::Ready(rows),
            metrics_enabled: false,
            pipeline_available: false,
            last_refreshed: Some("14:32:08".to_string()),
        }),
    )
}

fn processes_metrics_unknown() -> ViewModel {
    let mut rows = process_rows();
    for row in &mut rows {
        row.metrics = None;
    }
    base(
        Page::Processes,
        PageVm::Processes(ProcessesVm {
            rows: LoadState::Ready(rows),
            metrics_enabled: true,
            pipeline_available: true,
            last_refreshed: Some("14:32:08".to_string()),
        }),
    )
}

fn new_project_blocked() -> ViewModel {
    let mut vm = new_project(NewProjectStep::Details);
    if let Some(ModalVm::NewProject(state)) = &mut vm.modal {
        state.preflight = NewProjectPreflightVm {
            state: NewProjectPreflightState::Blocked,
            license_ok: false,
            message: Some(
                "Unity reports no active license. Sign in and activate a license before creating a project."
                    .to_string(),
            ),
            can_continue: false,
        };
        state.issues = vec![
            "The target folder already exists and is not empty: /home/ada/Games/OrbitalTwo"
                .to_string(),
        ];
    }
    vm
}

fn new_project_partial() -> ViewModel {
    let mut vm = new_project(NewProjectStep::Result);
    if let Some(ModalVm::NewProject(state)) = &mut vm.modal {
        state.result = Some(NewProjectResultVm {
            success: false,
            path: Some("/home/ada/Games/OrbitalTwo".to_string()),
            summary:
                "The project was created at /home/ada/Games/OrbitalTwo, but 1 setup step(s) failed."
                    .to_string(),
            failed_steps: vec!["cloudLink: NO_CLOUD".to_string()],
        });
    }
    vm
}

fn new_project(step: NewProjectStep) -> ViewModel {
    let mut vm = templates();
    vm.modal = Some(ModalVm::NewProject(Box::new(NewProjectVm {
        step,
        form: unity_hub_model::NewProjectFormVm {
            name: "OrbitalTwo".to_string(),
            parent_dir: "/home/ada/Games".to_string(),
            editor_version: "6000.3.15f1".to_string(),
            architecture: "x86_64".to_string(),
            template: "com.unity.template.urp-blank".to_string(),
            open_after_create: true,
            show_builtin_templates: false,
        },
        editors: LoadState::Ready(vec![NewProjectEditorVm {
            version: "6000.3.15f1".to_string(),
            architecture: "x86_64".to_string(),
            is_default: true,
        }]),
        templates: LoadState::Ready(template_rows()),
        selected_template: Some(template_detail()),
        preflight: NewProjectPreflightVm {
            state: NewProjectPreflightState::Ready,
            license_ok: true,
            message: None,
            can_continue: true,
        },
        issues: Vec::new(),
        command_preview: "unity projects create OrbitalTwo --output-dir /home/ada/Games --editor-version 6000.3.15f1 --template com.unity.template.urp-blank --architecture x86_64 --no-cloud --open --format ndjson --no-pager --no-banner --non-interactive".to_string(),
        result: (step == NewProjectStep::Result).then(|| NewProjectResultVm {
            success: true,
            path: Some("/home/ada/Games/OrbitalTwo".to_string()),
            summary: "Project created at /home/ada/Games/OrbitalTwo.".to_string(),
            failed_steps: Vec::new(),
        }),
        job: None,
    })));
    vm
}

fn template_authoring(mode: TemplateAuthoringMode, existing: bool) -> ViewModel {
    let mut vm = templates();
    vm.modal = Some(ModalVm::TemplateAuthoring(Box::new(TemplateAuthoringVm {
        mode,
        projects: vec![
            "/home/ada/Games/Orbital".to_string(),
            "/home/ada/Tools/LevelKit".to_string(),
        ],
        selected_project: Some("/home/ada/Games/Orbital".to_string()),
        form: TemplateFormVm {
            name: "com.ada.template.orbital".to_string(),
            display_name: "Orbital Starter".to_string(),
            description: "A 3D URP starter with the Orbital scene.".to_string(),
            version: "1.0.0".to_string(),
            output: if mode.is_pack() {
                "/home/ada/Games/orbital-template.tgz".to_string()
            } else {
                "/home/ada/.config/unityhub/templates".to_string()
            },
        },
        overwrite: false,
        overwrite_confirmed: existing,
        can_submit: !existing,
        issues: if existing {
            vec!["The archive already exists. Confirm overwrite to replace it.".to_string()]
        } else {
            Vec::new()
        },
        command_preview: if mode.is_pack() {
            "unity templates pack /home/ada/Games/Orbital --output /home/ada/Games/orbital-template.tgz --name com.ada.template.orbital --display-name 'Orbital Starter'".to_string()
        } else {
            "unity templates create /home/ada/Games/Orbital --name com.ada.template.orbital --display-name 'Orbital Starter'".to_string()
        },
        result: None,
        job: None,
    })));
    vm
}
