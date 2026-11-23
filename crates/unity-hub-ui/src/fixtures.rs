//! Deterministic view models for the design gallery and UI tests.
//!
//! These fixtures never touch the CLI or the controller. They are the contract
//! surface a designer can review without a Unity installation.

use unity_hub_model::{
    AccountCardVm, ArchitectureOptionVm, AuthVm, BadgeVm, BreakingChangeVm, ChangelogDataVm,
    ChangelogEntryVm, ChangelogSectionVm, ChangelogVm, CheckStatus, CliCardVm, CliChangelogVm,
    CliConnectionVm, CliDetailsVm, CliUpdateFlowVm, DiagnoseReportVm, DiagnoseTopic, DiagnosticsVm,
    DocsScopeVm, DoctorCheckVm, DoctorVm, EditorModulesVm, EditorRowVm, EditorsVm, EntryDocsVm,
    EnvironmentVm, ErrorActionKind, ErrorActionVm, InstallAdvancedVm, InstallCliVm,
    InstallEditorVm, JobPhase, JobVm, LicenseRowVm, LicenseVm, LoadState, ModalVm, ModuleOptionVm,
    NewProjectEditorVm, NewProjectPreflightState, NewProjectPreflightVm, NewProjectResultVm,
    NewProjectStep, NewProjectVm, NotificationActionState, NotificationVm, OverviewVm, Page,
    PageVm, ProcessMetricsVm, ProcessRowVm, ProcessesVm, ProjectRowVm, ProjectsVm, ProxyVm,
    PruneCandidateVm, PruneEditorsVm, PruneReportVm, ReleaseRowVm, RemoveProjectVm, RollbackCliVm,
    RunningEditorVm, SectionKind, SettingsVm, ShellVm, SkillClientVm, SkillForceVm,
    SkillInstallOptionsVm, SkillInstallVm, SkillOutcomeRowVm, SkillOutcomeVm, SkillRunVm, SkillsVm,
    TemplateAuthoringMode, TemplateAuthoringVm, TemplateDetailVm, TemplateFormVm,
    TemplateKindFilter, TemplateRowVm, TemplatesVm, TextScaleOptionVm, ToastVm, Tone, UiError,
    UpdateVm, UpgradeProjectVm, VerifyCheckVm, VerifyEditorVm, VerifyResultVm, ViewModel,
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
            name: "Overview · CLI update flow",
            view_model: overview_update_flow(),
        },
        Fixture {
            name: "Error · actionable (retry)",
            view_model: error_actionable(),
        },
        Fixture {
            name: "Error · unknown code",
            view_model: error_unknown_code(),
        },
        Fixture {
            name: "Advisory · CLI update",
            view_model: advisory_update(),
        },
        Fixture {
            name: "Advisory · plugin beta warning",
            view_model: advisory_plugin_beta(),
        },
        Fixture {
            name: "Advisory · unknown code",
            view_model: advisory_unknown(),
        },
        Fixture {
            name: "Advisory · dismissed",
            view_model: advisory_dismissed(),
        },
        Fixture {
            name: "Editors",
            view_model: editors(),
        },
        Fixture {
            name: "Editors · two builds of one version",
            view_model: editors_two_builds(),
        },
        Fixture {
            name: "Editors · default cleared",
            view_model: editors_default_cleared(),
        },
        Fixture {
            name: "Editors · advanced install options",
            view_model: editors_advanced(),
        },
        Fixture {
            name: "Editors · resume offered",
            view_model: editors_resume_offered(),
        },
        Fixture {
            name: "Editors · download-only finished",
            view_model: editors_download_only_done(),
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
            name: "Skills",
            view_model: skills(),
        },
        Fixture {
            name: "Skills · empty",
            view_model: skills_empty(),
        },
        Fixture {
            name: "Skills · installed",
            view_model: skills_installed(),
        },
        Fixture {
            name: "Skills · dry run",
            view_model: skills_dry_run(),
        },
        Fixture {
            name: "Skills · conflict",
            view_model: skills_conflict(),
        },
        Fixture {
            name: "Skills · loading",
            view_model: skills_loading(),
        },
        Fixture {
            name: "Skills · error",
            view_model: skills_error(),
        },
        Fixture {
            name: "Diagnostics",
            view_model: diagnostics(),
        },
        Fixture {
            name: "Diagnostics · not loaded",
            view_model: diagnostics_idle(),
        },
        Fixture {
            name: "Diagnostics · doctor not run",
            view_model: diagnostics_doctor_not_run(),
        },
        Fixture {
            name: "Diagnostics · loading",
            view_model: diagnostics_loading(),
        },
        Fixture {
            name: "Diagnostics · partial failure",
            view_model: diagnostics_partial_failure(),
        },
        Fixture {
            name: "Diagnostics · fully failed",
            view_model: diagnostics_failed(),
        },
        Fixture {
            name: "Changelog",
            view_model: changelog(),
        },
        Fixture {
            name: "Changelog · filter on",
            view_model: changelog_filtered(),
        },
        Fixture {
            name: "Changelog · docs action",
            view_model: changelog_docs_action(),
        },
        Fixture {
            name: "Changelog · docs failure",
            view_model: changelog_docs_failure(),
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
            name: "Modal · project upgrade (two builds)",
            view_model: upgrade_modal_two_builds(),
        },
        Fixture {
            name: "Modal · prune candidates",
            view_model: prune_modal(),
        },
        Fixture {
            name: "Modal · prune loading",
            view_model: prune_modal_loading(),
        },
        Fixture {
            name: "Modal · prune rescanning",
            view_model: prune_modal_rescanning(),
        },
        Fixture {
            name: "Modal · prune failed",
            view_model: prune_modal_failed(),
        },
        Fixture {
            name: "Modal · prune empty",
            view_model: prune_modal_empty(),
        },
        Fixture {
            name: "Modal · prune confirm",
            view_model: prune_modal_confirmed(),
        },
        Fixture {
            name: "Modal · prune remove missing",
            view_model: prune_modal_missing(),
        },
        Fixture {
            name: "Modal · prune running",
            view_model: prune_modal_running(),
        },
        Fixture {
            name: "Modal · prune force offer",
            view_model: prune_modal_force_offer(),
        },
        Fixture {
            name: "Modal · verify results",
            view_model: verify_modal(),
        },
        Fixture {
            name: "Modal · verify loading",
            view_model: verify_modal_loading(),
        },
        Fixture {
            name: "Modal · verify all passed",
            view_model: verify_modal_all_passed(),
        },
        Fixture {
            name: "Modal · verify failed read",
            view_model: verify_modal_failed_read(),
        },
        Fixture {
            name: "Modal · CLI rollback",
            view_model: rollback_modal(),
        },
        Fixture {
            name: "Modal · CLI rollback BAD_ARGS",
            view_model: rollback_modal_bad_args(),
        },
        Fixture {
            name: "Job · running",
            view_model: job_running(),
        },
        Fixture {
            name: "Job · failed",
            view_model: job_failed(),
        },
        Fixture {
            name: "Notifications · rail unread",
            view_model: notifications_rail_unread(),
        },
        Fixture {
            name: "Notifications · rail empty",
            view_model: notifications_rail_empty(),
        },
        Fixture {
            name: "Notifications · rail overflow",
            view_model: notifications_rail_overflow(),
        },
        Fixture {
            name: "Notifications · dismissed",
            view_model: notifications_dismissed(),
        },
        Fixture {
            name: "Notifications · toast",
            view_model: notifications_toast(),
        },
        Fixture {
            name: "Notifications · long message",
            view_model: notifications_long_message(),
        },
        Fixture {
            name: "Notification · action available",
            view_model: notification_action_available(),
        },
        Fixture {
            name: "Notification · confirming",
            view_model: notification_confirming(),
        },
        Fixture {
            name: "Notification · running",
            view_model: notification_running(),
        },
        Fixture {
            name: "Notification · failed",
            view_model: notification_failed(),
        },
        Fixture {
            name: "Notifications · job-stream advisory",
            view_model: notifications_job_stream_advisory(),
        },
    ]
}

fn shell(page: Page) -> ShellVm {
    ShellVm {
        page,
        cli: CliConnectionVm::Connected {
            version: Some("1.0.0".to_string()),
        },
        notifications: Vec::new(),
        unread: 0,
        rail_open: false,
        toast: None,
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

fn update_flow_idle() -> CliUpdateFlowVm {
    CliUpdateFlowVm {
        channel: "beta".to_string(),
        channel_options: vec!["stable".to_string(), "beta".to_string()],
        target_input: String::new(),
        changelog: LoadState::Idle,
        dry_run: LoadState::Idle,
        changelog_command: "unity self-update --changelog --format json".to_string(),
        apply_command: "unity self-update --yes --format ndjson".to_string(),
        rollback_command: "unity self-update --rollback --yes --format ndjson".to_string(),
        can_apply: false,
        apply_blocked_reason: Some(
            "Load the target release notes before applying the update.".to_string(),
        ),
    }
}

fn update_flow_ready() -> CliUpdateFlowVm {
    CliUpdateFlowVm {
        target_input: "1.0.0-beta.18".to_string(),
        changelog: LoadState::Ready(CliChangelogVm {
            version: "1.0.0-beta.18".to_string(),
            text: "## Added\n\n- `unity self-update --rollback` restores a kept CLI version.\n- `unity install --download-only` reports the download folder and registers no Editor.\n\n## Changed\n\n- `--rollback` refuses `--dry-run`, `--check`, `--changelog`, `--target` and `--channel` with `BAD_ARGS`."
                .to_string(),
        }),
        dry_run: LoadState::Ready(
            "{\n  \"currentVersion\": \"1.0.0-beta.17\",\n  \"targetVersion\": \"1.0.0-beta.18\",\n  \"channel\": \"beta\",\n  \"dryRun\": true\n}"
                .to_string(),
        ),
        can_apply: true,
        apply_blocked_reason: None,
        ..update_flow_idle()
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
            update_flow: update_flow_idle(),
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
            update_flow: update_flow_idle(),
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
                code: "CLI_NOT_FOUND".to_string(),
                message: "Could not resolve the Unity CLI.".to_string(),
                detail: Some("Checked PATH and the Hub install receipt.".to_string()),
                action: None,
            }),
            update: LoadState::Failed {
                error: UiError {
                    code: "UNEXPECTED_OUTPUT".to_string(),
                    message: "Update check failed.".to_string(),
                    detail: None,
                    action: None,
                },
                previous: None,
            },
            update_flow: update_flow_idle(),
            account: AccountCardVm {
                auth: AuthVm::Failed(UiError {
                    code: "AUTH_ERROR".to_string(),
                    message: "Not signed in.".to_string(),
                    detail: None,
                    action: None,
                }),
                license: LicenseVm::Failed(UiError {
                    code: "LICENSE_UNAVAILABLE".to_string(),
                    message: "License service unavailable.".to_string(),
                    detail: None,
                    action: None,
                }),
            },
            running: LoadState::Loading { previous: None },
            editor_count: None,
            project_count: None,
        }),
    )
}

fn overview_update_flow() -> ViewModel {
    let mut vm = overview();
    if let PageVm::Overview(overview) = &mut vm.page {
        overview.update = LoadState::Ready(UpdateVm {
            available: true,
            current_version: "1.0.0-beta.17".to_string(),
            latest_version: "1.0.0-beta.18".to_string(),
        });
        overview.update_flow = update_flow_ready();
    }
    vm
}

fn error_actionable() -> ViewModel {
    let mut vm = overview();
    if let PageVm::Overview(overview) = &mut vm.page {
        overview.cli = CliCardVm::Failed(UiError {
            code: "NETWORK_UNREACHABLE".to_string(),
            message: "A Unity service could not be reached.".to_string(),
            detail: Some(
                "Retry when the connection is back. The CLI is using the configured proxy `UNITY_PROXY=http://proxy.example:8080`; if that proxy is down, fix it or unset it first."
                    .to_string(),
            ),
            action: Some(ErrorActionVm {
                label: "Retry".to_string(),
                command: None,
                kind: ErrorActionKind::Retry("read:status".to_string()),
            }),
        });
    }
    vm
}

fn error_unknown_code() -> ViewModel {
    let mut vm = projects();
    if let PageVm::Projects(projects) = &mut vm.page {
        projects.projects = LoadState::Failed {
            error: UiError {
                code: "FUTURE_CLI_CODE".to_string(),
                message: "The Unity CLI returned an error this app does not know yet.".to_string(),
                detail: Some(
                    "The CLI message and code are shown verbatim; no action is guessed."
                        .to_string(),
                ),
                action: None,
            },
            previous: None,
        };
    }
    vm
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
                architecture: None,
                architecture_options: architecture_options(None),
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
                headless: false,
                headless_available: true,
                accept_eula: false,
                can_install: true,
                second_build_warning: None,
                preview: None,
                preview_note: None,
                preview_error: None,
                preview_loading: false,
                advanced: advanced_fixture(),
                command_preview: "unity install lts --yes --format ndjson --no-pager --no-banner --non-interactive".to_string(),
                resume_offer: None,
                download_report: None,
            },
            releases: LoadState::Ready(vec![
                ReleaseRowVm {
                    version: "6000.3.0f1".to_string(),
                    architecture: "x86_64".to_string(),
                    stream: "LTS".to_string(),
                    stream_tone: Tone::Success,
                    installed: false,
                },
                ReleaseRowVm {
                    version: "6000.2.0b1".to_string(),
                    architecture: "x86_64".to_string(),
                    stream: "Beta".to_string(),
                    stream_tone: Tone::Warning,
                    installed: true,
                },
            ]),
            prune: LoadState::Idle,
        }),
    )
}

fn architecture_options(selected: Option<&str>) -> Vec<ArchitectureOptionVm> {
    [None, Some("x86_64"), Some("arm64")]
        .into_iter()
        .map(|architecture| ArchitectureOptionVm {
            label: architecture.unwrap_or("Host default").to_string(),
            selected: architecture == selected,
            architecture: architecture.map(str::to_string),
        })
        .collect()
}

fn editors_two_builds() -> ViewModel {
    let mut vm = editors();
    if let PageVm::Editors(editors_vm) = &mut vm.page {
        editors_vm.installed = LoadState::Ready(vec![
            EditorRowVm {
                version: "6000.1.1f1".to_string(),
                architecture: "arm64".to_string(),
                is_default: true,
                modules: vec!["mac-il2cpp".to_string()],
            },
            EditorRowVm {
                version: "6000.1.1f1".to_string(),
                architecture: "x86_64".to_string(),
                is_default: false,
                modules: vec!["linux-il2cpp".to_string(), "linux-server".to_string()],
            },
        ]);
        editors_vm.install = InstallEditorVm {
            version_input: "6000.1.1f1".to_string(),
            architecture: Some("x86_64".to_string()),
            architecture_options: architecture_options(Some("x86_64")),
            modules: LoadState::Ready(Vec::new()),
            available_count: 0,
            headless: false,
            headless_available: true,
            accept_eula: true,
            can_install: true,
            second_build_warning: Some(
                "Unity 6000.1.1f1 is already installed for arm64. Installing the x86_64 build keeps both on disk, and projects pinned to the existing build keep using it."
                    .to_string(),
            ),
            preview: Some(unity_hub_model::InstallPreviewVm {
                editor_version: "6000.1.1f1".to_string(),
                architecture: Some("x86_64".to_string()),
                download_size: "6.4 GB".to_string(),
                module_count: 0,
            }),
            preview_note: None,
            preview_error: None,
            preview_loading: false,
            advanced: advanced_fixture(),
            command_preview: "unity install 6000.1.1f1 --yes --architecture x86_64 --accept-eula --format ndjson".to_string(),
            resume_offer: None,
            download_report: None,
        };
    }
    vm
}

/// No stored default: no row shows the badge.
fn editors_default_cleared() -> ViewModel {
    let mut vm = editors();
    if let PageVm::Editors(editors_vm) = &mut vm.page {
        if let LoadState::Ready(rows) = &mut editors_vm.installed {
            for row in rows {
                row.is_default = false;
            }
        }
    }
    vm
}

fn advanced_fixture() -> InstallAdvancedVm {
    InstallAdvancedVm {
        expanded: false,
        resume: false,
        download_only: false,
        no_wait: false,
        timeout_input: String::new(),
        set_default: false,
        retries_input: String::new(),
        conflict_note: None,
        timeout_error: None,
        retries_error: None,
        can_install: true,
    }
}

fn editors_advanced() -> ViewModel {
    let mut vm = editors();
    if let PageVm::Editors(editors_vm) = &mut vm.page {
        editors_vm.install.advanced = InstallAdvancedVm {
            expanded: true,
            resume: true,
            download_only: false,
            no_wait: true,
            timeout_input: String::new(),
            set_default: false,
            retries_input: "2".to_string(),
            conflict_note: Some(
                "No-wait fails immediately when another install holds the lock, so it cleared the timeout. The CLI refuses both at once."
                    .to_string(),
            ),
            timeout_error: None,
            retries_error: None,
            can_install: true,
        };
        editors_vm.install.command_preview =
            "unity install lts --yes --resume --no-wait --format ndjson".to_string();
    }
    vm
}

fn editors_resume_offered() -> ViewModel {
    let mut vm = editors();
    if let PageVm::Editors(editors_vm) = &mut vm.page {
        editors_vm.install.resume_offer = Some(unity_hub_model::ResumeInstallVm {
            version: "6000.3.15f1".to_string(),
            architecture: "x86_64".to_string(),
            reason: "The CLI kept the partial download at /home/ada/Unity/Cache/6000.3.15f1."
                .to_string(),
            can_resume: true,
        });
    }
    vm
}

fn editors_download_only_done() -> ViewModel {
    let mut vm = editors();
    if let PageVm::Editors(editors_vm) = &mut vm.page {
        editors_vm.install.advanced = InstallAdvancedVm {
            expanded: true,
            download_only: true,
            ..advanced_fixture()
        };
        editors_vm.install.download_report = Some(
            "Files downloaded to /home/ada/Unity/downloads. Nothing was installed or registered, so no Editor was added."
                .to_string(),
        );
    }
    vm
}

fn prune_report_fixture() -> PruneReportVm {
    PruneReportVm {
        unused: vec![
            PruneCandidateVm {
                version: "6000.3.27f2".to_string(),
                architecture: "x86_64".to_string(),
                path: "/home/ada/Unity/6000.3.27f2".to_string(),
                size: Some("8.1 GB".to_string()),
                reason: "Reported unused by `unity editors prune`.".to_string(),
            },
            PruneCandidateVm {
                version: "2022.3.10f1".to_string(),
                architecture: "arm64".to_string(),
                path: "/home/ada/Unity/2022.3.10f1".to_string(),
                size: Some("5.3 GB".to_string()),
                reason: "Reported unused by `unity editors prune`.".to_string(),
            },
        ],
        missing: vec![PruneCandidateVm {
            version: "6000.0.0b1".to_string(),
            architecture: "x86_64".to_string(),
            path: "/Volumes/External/Unity/6000.0.0b1".to_string(),
            size: None,
            reason: "Reported missing by `unity editors prune --remove-missing`.".to_string(),
        }],
        reclaimable: "13.4 GB".to_string(),
        removed: false,
    }
}

fn prune_modal_baseline(report: LoadState<PruneReportVm>) -> PruneEditorsVm {
    PruneEditorsVm {
        report,
        confirmed: false,
        missing_confirmed: false,
        force: false,
        force_available: false,
        scan_command: "unity editors prune --remove-missing --format json --no-pager --no-banner --non-interactive".to_string(),
        remove_command: "unity editors prune --remove --yes --format ndjson --no-pager --no-banner --non-interactive".to_string(),
        missing_command: "unity editors prune --remove-missing --yes --format ndjson --no-pager --no-banner --non-interactive".to_string(),
        remove_label: "Remove 2 unused Editors (13.4 GB)".to_string(),
        missing_label: "Remove 1 missing registry entry (no files deleted)".to_string(),
        confirm_label: "I understand these Editors and their files will be uninstalled: Unity 6000.3.27f2 (x86_64, 8.1 GB), Unity 2022.3.10f1 (arm64, 5.3 GB).".to_string(),
        missing_confirm_label: "I understand only these registry entries are dropped; no files are deleted: Unity 6000.0.0b1 (x86_64).".to_string(),
        can_remove: false,
        can_remove_missing: false,
        error: None,
        job_running: false,
    }
}

fn prune_modal_with(
    report: LoadState<PruneReportVm>,
    configure: impl FnOnce(&mut PruneEditorsVm),
) -> ViewModel {
    let mut vm = editors();
    let mut prune = prune_modal_baseline(report);
    configure(&mut prune);
    vm.modal = Some(ModalVm::PruneEditors(prune));
    vm
}

fn prune_modal() -> ViewModel {
    prune_modal_with(LoadState::Ready(prune_report_fixture()), |_| {})
}

fn prune_modal_loading() -> ViewModel {
    prune_modal_with(LoadState::Loading { previous: None }, |_| {})
}

fn prune_modal_rescanning() -> ViewModel {
    prune_modal_with(
        LoadState::Loading {
            previous: Some(prune_report_fixture()),
        },
        |_| {},
    )
}

fn prune_modal_failed() -> ViewModel {
    prune_modal_with(
        LoadState::Failed {
            error: UiError {
                code: "NETWORK_UNREACHABLE".to_string(),
                message: "The prune report could not be read.".to_string(),
                detail: Some("Retry when the connection is back.".to_string()),
                action: None,
            },
            previous: Some(prune_report_fixture()),
        },
        |_| {},
    )
}

fn prune_modal_empty() -> ViewModel {
    prune_modal_with(
        LoadState::Ready(PruneReportVm {
            unused: Vec::new(),
            missing: Vec::new(),
            reclaimable: "0 B".to_string(),
            removed: false,
        }),
        |_| {},
    )
}

fn prune_modal_confirmed() -> ViewModel {
    prune_modal_with(LoadState::Ready(prune_report_fixture()), |prune| {
        prune.confirmed = true;
        prune.force_available = true;
        prune.can_remove = true;
    })
}

fn prune_modal_missing() -> ViewModel {
    prune_modal_with(LoadState::Ready(prune_report_fixture()), |prune| {
        prune.missing_confirmed = true;
        prune.can_remove_missing = true;
    })
}

fn prune_modal_running() -> ViewModel {
    prune_modal_with(LoadState::Ready(prune_report_fixture()), |prune| {
        prune.confirmed = true;
        prune.job_running = true;
        prune.can_remove = false;
        prune.can_remove_missing = false;
    })
}

fn prune_modal_force_offer() -> ViewModel {
    prune_modal_with(LoadState::Ready(prune_report_fixture()), |prune| {
        prune.confirmed = true;
        prune.force_available = true;
        prune.force = true;
        prune.can_remove = true;
        prune.remove_command = "unity editors prune --remove --yes --force --format ndjson --no-pager --no-banner --non-interactive".to_string();
        prune.error = Some(UiError {
            code: "EDITORS_PRUNE_EDITOR_CHECK_FAILED".to_string(),
            message: "Could not detect running Editors.".to_string(),
            detail: Some(
                "Review the running Editors, then retry with --force only if you want to remove them anyway."
                    .to_string(),
            ),
            action: None,
        });
    })
}

fn verify_modal_baseline(state: LoadState<VerifyResultVm>, show_raw: bool) -> VerifyEditorVm {
    VerifyEditorVm {
        version: "6000.6.6f1".to_string(),
        architecture: "x86_64".to_string(),
        command: "unity editors verify 6000.6.6f1 --architecture x86_64 --format json --no-pager --no-banner --non-interactive".to_string(),
        state,
        show_raw,
    }
}

fn verify_modal_with(state: LoadState<VerifyResultVm>, show_raw: bool) -> ViewModel {
    let mut vm = editors();
    vm.modal = Some(ModalVm::VerifyEditor(verify_modal_baseline(
        state, show_raw,
    )));
    vm
}

fn verify_failed_result() -> VerifyResultVm {
    VerifyResultVm {
        version: "6000.6.6f1".to_string(),
        architecture: "x86_64".to_string(),
        ok: false,
        checks: vec![
            VerifyCheckVm {
                component: "Editor".to_string(),
                kind: "Editor".to_string(),
                status: CheckStatus::Pass,
                path: Some("/home/ada/Unity/6000.6.6f1/Editor/Unity".to_string()),
                repair_command: None,
            },
            VerifyCheckVm {
                component: "Windows Mono".to_string(),
                kind: "Module".to_string(),
                status: CheckStatus::Warn,
                path: Some("/home/ada/Unity/6000.6.6f1/Editor/Data/PlaybackEngines/WindowsStandaloneSupport".to_string()),
                repair_command: None,
            },
            VerifyCheckVm {
                component: "iOS".to_string(),
                kind: "Module".to_string(),
                status: CheckStatus::Fail,
                path: Some("/home/ada/Unity/6000.6.6f1/Editor/Data/PlaybackEngines/iOSSupport".to_string()),
                repair_command: Some("unity install-modules --editor-version 6000.6.6f1 --architecture x86_64 -m ios --yes".to_string()),
            },
            VerifyCheckVm {
                component: "Linux IL2CPP".to_string(),
                kind: "Module".to_string(),
                status: CheckStatus::Unknown,
                path: None,
                repair_command: None,
            },
        ],
        failed_count: 1,
        repair_command: Some("unity install-modules --editor-version 6000.6.6f1 --architecture x86_64 -m ios --yes".to_string()),
        raw: "{\n  \"success\": false,\n  \"command\": \"editors verify\",\n  \"data\": {\n    \"version\": \"6000.6.6f1\",\n    \"architecture\": \"x86_64\",\n    \"ok\": false,\n    \"components\": [\n      { \"component\": \"iOS\", \"status\": \"missing\" }\n    ]\n  },\n  \"errors\": [\n    { \"code\": \"EDITORS_VERIFY_FAILED\", \"message\": \"1 component is missing.\" }\n  ]\n}".to_string(),
    }
}

fn verify_modal() -> ViewModel {
    verify_modal_with(LoadState::Ready(verify_failed_result()), true)
}

fn verify_modal_loading() -> ViewModel {
    verify_modal_with(LoadState::Loading { previous: None }, false)
}

fn verify_modal_all_passed() -> ViewModel {
    verify_modal_with(
        LoadState::Ready(VerifyResultVm {
            version: "6000.6.6f1".to_string(),
            architecture: "x86_64".to_string(),
            ok: true,
            checks: Vec::new(),
            failed_count: 0,
            repair_command: None,
            raw: "{\"success\":true,\"data\":{\"ok\":true,\"components\":[]}}".to_string(),
        }),
        false,
    )
}

fn verify_modal_failed_read() -> ViewModel {
    verify_modal_with(
        LoadState::Failed {
            error: UiError {
                code: "EDITOR_NOT_FOUND".to_string(),
                message: "The Editor build is no longer installed.".to_string(),
                detail: Some("Refresh the Editors list and choose an installed build.".to_string()),
                action: None,
            },
            previous: None,
        },
        false,
    )
}

fn rollback_modal() -> ViewModel {
    let mut vm = overview();
    vm.modal = Some(ModalVm::RollbackCli(RollbackCliVm {
        version_input: "1.0.0-beta.16".to_string(),
        restore_label: "Unity CLI 1.0.0-beta.16".to_string(),
        confirmed: true,
        can_rollback: true,
        command: "unity self-update --rollback 1.0.0-beta.16 --yes --format ndjson --no-pager --no-banner --non-interactive".to_string(),
        error: None,
        job_running: false,
    }));
    vm
}

fn rollback_modal_bad_args() -> ViewModel {
    let mut vm = rollback_modal();
    if let Some(ModalVm::RollbackCli(rollback)) = &mut vm.modal {
        rollback.error = Some(UiError {
            code: "BAD_ARGS".to_string(),
            message: "The CLI refused the rollback arguments.".to_string(),
            detail: Some(
                "Rollback cannot be combined with --check, --changelog, --target, --channel or --dry-run. No binary changed."
                    .to_string(),
            ),
            action: Some(ErrorActionVm {
                label: "Copy the command".to_string(),
                command: Some("unity self-update --rollback".to_string()),
                kind: ErrorActionKind::ShowCommand,
            }),
        });
    }
    vm
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
                    architecture: "arm64".to_string(),
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
                    architecture: "x86_64".to_string(),
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
                    architecture: "x86_64".to_string(),
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

fn doctor_fixture() -> DoctorVm {
    DoctorVm {
        platform: "linux".to_string(),
        arch: "x86_64".to_string(),
        runtime_version: "8.0.4".to_string(),
        node_version: "22.11.0".to_string(),
        cli_version: "1.0.0-beta.17".to_string(),
        install_path: "/home/ada/.unity".to_string(),
        log_directory: "/home/ada/.unity/logs".to_string(),
        checks: vec![
            DoctorCheckVm {
                id: "path-available".to_string(),
                label: "Path available".to_string(),
                status: CheckStatus::Pass,
                detail: Some("Directory: /home/ada/.local/bin".to_string()),
            },
            DoctorCheckVm {
                id: "git-credential-helper".to_string(),
                label: "Git credential helper".to_string(),
                status: CheckStatus::Warn,
                detail: Some("Helper: none".to_string()),
            },
            DoctorCheckVm {
                id: "disk-space".to_string(),
                label: "Disk space".to_string(),
                status: CheckStatus::Fail,
                detail: Some("Available: 2.1 GB · Required: 15 GB".to_string()),
            },
            DoctorCheckVm {
                id: "fex-emu".to_string(),
                label: "FEX emu".to_string(),
                status: CheckStatus::Info,
                detail: Some("Detected: false".to_string()),
            },
            DoctorCheckVm {
                id: "future-check".to_string(),
                label: "Future check".to_string(),
                status: CheckStatus::Unknown,
                detail: None,
            },
        ],
        auth: Some("Logged in: true · User: ada@example.com".to_string()),
        license: Some("Active: true · Product: Unity Pro".to_string()),
        raw: "{\n  \"platform\": \"linux\",\n  \"arch\": \"x86_64\",\n  \"runtimeVersion\": \"8.0.4\",\n  \"cliVersion\": \"1.0.0-beta.17\",\n  \"checks\": [\n    { \"id\": \"path-available\", \"status\": \"pass\" },\n    { \"id\": \"git-credential-helper\", \"status\": \"warn\" },\n    { \"id\": \"disk-space\", \"status\": \"fail\" }\n  ]\n}"
            .to_string(),
    }
}

fn environment_fixture() -> EnvironmentVm {
    EnvironmentVm {
        user_data_path: "/home/ada/.config/Unity".to_string(),
        editor_install_path: "/home/ada/Unity/Editors".to_string(),
        download_cache_path: "/home/ada/Unity/Cache".to_string(),
        config_path: "/home/ada/.unity".to_string(),
        cli_version: "1.0.0-beta.17".to_string(),
        hub_version: "0.2.1".to_string(),
        proxy: Some(ProxyVm {
            url: "http://proxy.example:8080".to_string(),
            source: "env".to_string(),
            auth_source: "keyring".to_string(),
            no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
        }),
    }
}

fn diagnose_fixture(topic: DiagnoseTopic) -> DiagnoseReportVm {
    let text = match topic {
        DiagnoseTopic::Proxy => {
            "Proxy diagnostic (redacted)\n\nResolved proxy: http://proxy.example:8080\nSource: env\nAuth source: keyring\nNo proxy: localhost, 127.0.0.1\n\nCA bundle: system\nPAC: not configured\nCredentials: present in the OS credential store\n\nA copy was written to /home/ada/.unity/logs/proxy-report.txt"
        }
        DiagnoseTopic::Accelerator => {
            "Unity Accelerator diagnostic\n\nResolved endpoint: 127.0.0.1:10080\nSource: settings\nDisabled: no\n\nProject cache server: disabled\nReachability: connection refused (ECONNREFUSED, 5.0 s)\n"
        }
        DiagnoseTopic::Update => {
            "Unity CLI update diagnostic\n\nChannel: beta\nManifest: https://services.api.unity.com/unity/cli/manifest.json\nCurrent: 1.0.0-beta.17\nLatest: 1.0.0-beta.17\nHolding update back: none\n"
        }
    };

    DiagnoseReportVm {
        topic,
        text: text.to_string(),
    }
}

fn skill_client(client: &str, label: &str, path: &str, installed: bool) -> SkillClientVm {
    SkillClientVm {
        client: client.to_string(),
        label: label.to_string(),
        path: path.to_string(),
        installed,
    }
}

fn skill_install(path: &str, client: &str, skill: &str, status: &str) -> SkillInstallVm {
    SkillInstallVm {
        path: path.to_string(),
        client: client.to_string(),
        skill: skill.to_string(),
        status: status.to_string(),
    }
}

fn skill_outcome(
    path: &str,
    client: &str,
    skill: &str,
    outcome: SkillOutcomeVm,
    reason: Option<&str>,
) -> SkillOutcomeRowVm {
    SkillOutcomeRowVm {
        path: path.to_string(),
        client: client.to_string(),
        skill: skill.to_string(),
        outcome,
        skip_reason: reason.map(str::to_string),
    }
}

fn skills_options() -> SkillInstallOptionsVm {
    SkillInstallOptionsVm {
        local: false,
        overwrite: false,
        dry_run: false,
        skill_input: String::new(),
        skill_input_error: None,
        all_command:
            "unity skill install --all --format ndjson --no-pager --no-banner --non-interactive"
                .to_string(),
    }
}

fn skills_vm(
    clients: LoadState<Vec<SkillClientVm>>,
    installs: LoadState<Vec<SkillInstallVm>>,
    result: Option<SkillRunVm>,
) -> SkillsVm {
    SkillsVm {
        clients,
        installs,
        install: skills_options(),
        result,
        refresh_label: "Preview refresh (dry run)".to_string(),
        refresh_previewed: false,
        refresh_hint:
            "The first refresh is a dry run: it changes nothing and labels every row would-*."
                .to_string(),
        refresh_command:
            "unity skill refresh --yes --dry-run --format ndjson --no-pager --no-banner --non-interactive"
                .to_string(),
        project_context: Some("/home/ada/Games/Orbital".to_string()),
        local_available: true,
        local_disabled_reason: None,
        job_running: false,
    }
}

fn skills() -> ViewModel {
    base(
        Page::Skills,
        PageVm::Skills(skills_vm(
            LoadState::Ready(vec![
                skill_client(
                    "claude-code",
                    "Claude Code",
                    "/home/ada/.claude/skills",
                    true,
                ),
                skill_client("cursor", "Cursor", "/home/ada/.cursor/skills", true),
                skill_client("codex", "Codex", "/home/ada/.codex/skills", false),
                skill_client(
                    "claude-desktop",
                    "Claude Desktop",
                    "/home/ada/Library/Application Support/Claude/skills",
                    false,
                ),
            ]),
            LoadState::Ready(vec![
                skill_install(
                    "/home/ada/.claude/skills/unity-cli/SKILL.md",
                    "claude-code",
                    "unity-cli",
                    "installed",
                ),
                skill_install(
                    "/home/ada/.cursor/skills/unity-new-project/SKILL.md",
                    "cursor",
                    "new-unity-project",
                    "installed",
                ),
            ]),
            None,
        )),
    )
}

fn skills_empty() -> ViewModel {
    let mut vm = skills();
    if let PageVm::Skills(skills) = &mut vm.page {
        skills.clients = LoadState::Ready(vec![
            skill_client(
                "claude-code",
                "Claude Code",
                "/home/ada/.claude/skills",
                false,
            ),
            skill_client("cursor", "Cursor", "/home/ada/.cursor/skills", false),
            skill_client("", "Unknown client", "", false),
        ]);
        skills.installs = LoadState::Ready(Vec::new());
        skills.project_context = None;
        skills.local_available = false;
        skills.local_disabled_reason = Some(
            "`--local` writes into the current project. The Hub was not launched from a Unity project, so it has no project context to write into."
                .to_string(),
        );
    }
    vm
}

fn skills_installed() -> ViewModel {
    let mut vm = skills();
    if let PageVm::Skills(skills) = &mut vm.page {
        if let LoadState::Ready(clients) = &mut skills.clients {
            for client in clients {
                client.installed = true;
            }
        }
        skills.result = Some(SkillRunVm {
            dry_run: false,
            rows: vec![
                skill_outcome(
                    "/home/ada/.claude/skills/unity-cli/SKILL.md",
                    "claude-code",
                    "unity-cli",
                    SkillOutcomeVm::Refreshed,
                    None,
                ),
                skill_outcome(
                    "/home/ada/.cursor/skills/unity-new-project/SKILL.md",
                    "cursor",
                    "new-unity-project",
                    SkillOutcomeVm::UpToDate,
                    None,
                ),
                skill_outcome(
                    "/home/ada/.codex/skills/unity-cli/SKILL.md",
                    "codex",
                    "unity-cli",
                    SkillOutcomeVm::Pruned,
                    Some("The skill no longer exists upstream."),
                ),
                skill_outcome(
                    "/home/ada/.windsurf/skills/unity-cli/SKILL.md",
                    "windsurf",
                    "unity-cli",
                    SkillOutcomeVm::Skipped,
                    Some("The copy is older and --yes was not passed."),
                ),
            ],
            force: None,
        });
        skills.refresh_label = "Refresh now".to_string();
        skills.refresh_previewed = true;
        skills.refresh_hint =
            "The next refresh applies for real and reports one row per tracked install."
                .to_string();
        skills.refresh_command =
            "unity skill refresh --yes --format ndjson --no-pager --no-banner --non-interactive"
                .to_string();
    }
    vm
}

fn skills_dry_run() -> ViewModel {
    let mut vm = skills();
    if let PageVm::Skills(skills) = &mut vm.page {
        skills.result = Some(SkillRunVm {
            dry_run: true,
            rows: vec![
                skill_outcome(
                    "/home/ada/.claude/skills/unity-cli/SKILL.md",
                    "claude-code",
                    "unity-cli",
                    SkillOutcomeVm::WouldUpdate,
                    None,
                ),
                skill_outcome(
                    "/home/ada/.cursor/skills/unity-new-project/SKILL.md",
                    "cursor",
                    "new-unity-project",
                    SkillOutcomeVm::WouldSkip,
                    Some("The copy is older and --yes was not passed."),
                ),
                skill_outcome(
                    "/home/ada/.codex/skills/unity-cli/SKILL.md",
                    "codex",
                    "unity-cli",
                    SkillOutcomeVm::WouldPrune,
                    Some("The skill is no longer installed for this client."),
                ),
            ],
            force: None,
        });
    }
    vm
}

fn skills_conflict() -> ViewModel {
    let mut vm = skills();
    if let PageVm::Skills(skills) = &mut vm.page {
        skills.result = Some(SkillRunVm {
            dry_run: false,
            rows: vec![
                skill_outcome(
                    "/home/ada/.claude/skills/unity-cli/SKILL.md",
                    "claude-code",
                    "unity-cli",
                    SkillOutcomeVm::Refreshed,
                    None,
                ),
                skill_outcome(
                    "/home/ada/.vscode/skills/unity-cli/SKILL.md",
                    "vscode",
                    "unity-cli",
                    SkillOutcomeVm::Conflict,
                    Some("The CLI did not write this copy."),
                ),
            ],
            force: Some(SkillForceVm {
                explanation: "The Unity CLI did not write the `vscode` copy, so it left it alone rather than overwrite it. `--force` takes ownership of that copy and replaces it with the CLI's own. Only run it if you know who wrote it."
                    .to_string(),
                command: "unity skill install vscode --force --format ndjson --no-pager --no-banner --non-interactive".to_string(),
                scope: "`vscode`".to_string(),
            }),
        });
    }
    vm
}

fn skills_loading() -> ViewModel {
    let mut vm = skills();
    if let PageVm::Skills(skills) = &mut vm.page {
        skills.clients = LoadState::Loading { previous: None };
        skills.installs = LoadState::Loading {
            previous: Some(Vec::new()),
        };
    }
    vm
}

fn skills_error() -> ViewModel {
    let mut vm = skills();
    if let PageVm::Skills(skills) = &mut vm.page {
        skills.clients = LoadState::Failed {
            error: UiError {
                code: "UNEXPECTED_OUTPUT".to_string(),
                message: "Could not parse the supported clients list.".to_string(),
                detail: Some("`unity skill install --list` returned an unknown shape.".to_string()),
                action: Some(ErrorActionVm {
                    label: "Retry".to_string(),
                    command: None,
                    kind: ErrorActionKind::Retry("read:skill-clients".to_string()),
                }),
            },
            previous: None,
        };
        skills.installs = LoadState::Failed {
            error: UiError {
                code: "NETWORK_UNREACHABLE".to_string(),
                message: "A Unity service could not be reached.".to_string(),
                detail: Some(
                    "Retry when the connection is back; the tracked installs read is independent of the client list."
                        .to_string(),
                ),
                action: Some(ErrorActionVm {
                    label: "Retry".to_string(),
                    command: None,
                    kind: ErrorActionKind::Retry("read:skill-installs".to_string()),
                }),
            },
            previous: None,
        };
    }
    vm
}

fn diagnostics() -> ViewModel {
    base(
        Page::Diagnostics,
        PageVm::Diagnostics(DiagnosticsVm {
            doctor: LoadState::Ready(doctor_fixture()),
            environment: LoadState::Ready(environment_fixture()),
            proxy_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Proxy)),
            accelerator_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Accelerator)),
            update_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Update)),
            can_copy: true,
        }),
    )
}

fn diagnostics_idle() -> ViewModel {
    base(
        Page::Diagnostics,
        PageVm::Diagnostics(DiagnosticsVm {
            doctor: LoadState::Idle,
            environment: LoadState::Idle,
            proxy_report: LoadState::Idle,
            accelerator_report: LoadState::Idle,
            update_report: LoadState::Idle,
            can_copy: false,
        }),
    )
}

/// The page's normal state: the automatic reads have loaded, while
/// `unity doctor` waits for an explicit Run because its probe can prompt.
fn diagnostics_doctor_not_run() -> ViewModel {
    base(
        Page::Diagnostics,
        PageVm::Diagnostics(DiagnosticsVm {
            doctor: LoadState::Idle,
            environment: LoadState::Ready(environment_fixture()),
            proxy_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Proxy)),
            accelerator_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Accelerator)),
            update_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Update)),
            can_copy: true,
        }),
    )
}

fn diagnostics_loading() -> ViewModel {
    base(
        Page::Diagnostics,
        PageVm::Diagnostics(DiagnosticsVm {
            doctor: LoadState::Loading {
                previous: Some(doctor_fixture()),
            },
            environment: LoadState::Loading { previous: None },
            proxy_report: LoadState::Loading { previous: None },
            accelerator_report: LoadState::Loading { previous: None },
            update_report: LoadState::Loading { previous: None },
            can_copy: true,
        }),
    )
}

fn diagnostics_partial_failure() -> ViewModel {
    base(
        Page::Diagnostics,
        PageVm::Diagnostics(DiagnosticsVm {
            doctor: LoadState::Ready(doctor_fixture()),
            environment: LoadState::Failed {
                error: UiError {
                    code: "NETWORK_UNREACHABLE".to_string(),
                    message: "A Unity service could not be reached.".to_string(),
                    detail: Some(
                        "The CLI is using the configured proxy `UNITY_PROXY=http://proxy.example:8080`."
                            .to_string(),
                    ),
                    action: Some(ErrorActionVm {
                        label: "Retry".to_string(),
                        command: None,
                        kind: ErrorActionKind::Retry("read:environment".to_string()),
                    }),
                },
                previous: Some(environment_fixture()),
            },
            proxy_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Proxy)),
            accelerator_report: LoadState::Failed {
                error: UiError {
                    code: "TIMEOUT".to_string(),
                    message: "The Accelerator report timed out.".to_string(),
                    detail: None,
                    action: Some(ErrorActionVm {
                        label: "Retry".to_string(),
                        command: None,
                        kind: ErrorActionKind::Retry("read:diagnose-accelerator".to_string()),
                    }),
                },
                previous: None,
            },
            update_report: LoadState::Ready(diagnose_fixture(DiagnoseTopic::Update)),
            can_copy: true,
        }),
    )
}

fn failed_state<T>(kind: &str) -> LoadState<T> {
    LoadState::Failed {
        error: UiError {
            code: "NETWORK_UNREACHABLE".to_string(),
            message: "A Unity service could not be reached.".to_string(),
            detail: Some(
                "Retry when the connection is back. The CLI is using the configured proxy."
                    .to_string(),
            ),
            action: Some(ErrorActionVm {
                label: "Retry".to_string(),
                command: None,
                kind: ErrorActionKind::Retry(kind.to_string()),
            }),
        },
        previous: None,
    }
}

fn diagnostics_failed() -> ViewModel {
    base(
        Page::Diagnostics,
        PageVm::Diagnostics(DiagnosticsVm {
            doctor: failed_state("read:doctor"),
            environment: failed_state("read:environment"),
            proxy_report: failed_state("read:diagnose-proxy"),
            accelerator_report: failed_state("read:diagnose-accelerator"),
            update_report: failed_state("read:diagnose-update"),
            can_copy: false,
        }),
    )
}

fn changelog() -> ViewModel {
    base(
        Page::Changelog,
        PageVm::Changelog(ChangelogVm {
            loaded: true,
            versions: vec!["1.0.0-beta.16".to_string(), "1.0.0-beta.17".to_string()],
            selected_target: Some("1.0.0-beta.17".to_string()),
            show_raw: false,
            app_impact_only: false,
            docs_scope: DocsScopeVm::Automatic,
            docs_package: String::new(),
            docs_scope_note: None,
            state: LoadState::Ready(changelog_data()),
            raw: LoadState::Idle,
        }),
    )
}

fn changelog_data() -> ChangelogDataVm {
    ChangelogDataVm {
        version: "1.0.0-beta.17".to_string(),
        source: "unity-changelog".to_string(),
        source_detail: "Unity CLI 1.0.0-beta.17 release notes".to_string(),
        entry_count: 5,
        app_impact_count: 3,
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
                    changelog_entry(
                        "`unity install-modules --architecture` now selects the build the row names.",
                        true,
                        EntryDocsVm::Idle {
                            topic: "install-modules".to_string(),
                            scope: DocsScopeVm::Cli,
                        },
                    ),
                    changelog_entry(
                        "NDJSON progress includes module download totals.",
                        false,
                        EntryDocsVm::Unavailable,
                    ),
                    changelog_entry(
                        "`unity docs Start` opens the common class page for a bare name.",
                        true,
                        EntryDocsVm::Ready {
                            topic: "MonoBehaviour.Start".to_string(),
                            url: "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/MonoBehaviour.Start.html".to_string(),
                            search: false,
                        },
                    ),
                ],
            },
            ChangelogSectionVm {
                title: "Changed".to_string(),
                kind: SectionKind::Changed,
                entries: vec![
                    changelog_entry(
                        "`--non-interactive` is now the default for machine output.",
                        true,
                        EntryDocsVm::Unavailable,
                    ),
                    changelog_entry(
                        "`MonoBehaviour.Start` is documented with a clearer example.",
                        false,
                        EntryDocsVm::Idle {
                            topic: "MonoBehaviour.Start".to_string(),
                            scope: DocsScopeVm::ScriptReference,
                        },
                    ),
                ],
            },
        ],
    }
}

fn changelog_entry(text: &str, app_impact: bool, docs: EntryDocsVm) -> ChangelogEntryVm {
    ChangelogEntryVm {
        text: text.to_string(),
        app_impact,
        docs,
    }
}

fn changelog_filtered() -> ViewModel {
    let mut vm = changelog();
    if let PageVm::Changelog(changelog) = &mut vm.page {
        changelog.app_impact_only = true;
    }
    vm
}

fn changelog_docs_action() -> ViewModel {
    let mut vm = changelog();
    if let PageVm::Changelog(changelog) = &mut vm.page {
        if let LoadState::Ready(data) = &mut changelog.state {
            data.sections[0].entries[0].docs = EntryDocsVm::Idle {
                topic: "install-modules".to_string(),
                scope: DocsScopeVm::Cli,
            };
            data.sections[0].entries[2].docs = EntryDocsVm::Ready {
                topic: "MonoBehaviour.Start".to_string(),
                url: "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/MonoBehaviour.Start.html"
                    .to_string(),
                search: true,
            };
        }
    }
    vm
}

fn changelog_docs_failure() -> ViewModel {
    let mut vm = changelog();
    if let PageVm::Changelog(changelog) = &mut vm.page {
        if let LoadState::Ready(data) = &mut changelog.state {
            data.sections[0].entries[0].docs = EntryDocsVm::Failed {
                topic: "install-modules".to_string(),
                message: "The Unity CLI could not reach the documentation index.".to_string(),
            };
            data.sections[0].entries[2].docs = EntryDocsVm::Missing {
                topic: "MonoBehaviour.Start".to_string(),
            };
        }
    }
    vm
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
        architecture: "x86_64".to_string(),
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
        retries_input: "2".to_string(),
        download_directory: String::new(),
        retries_error: None,
        command_preview: "unity install-modules --editor-version 6000.1.1f1 --yes --architecture x86_64 -m webgl --accept-eula --retries 2 --format ndjson".to_string(),
    }));
    vm
}

fn upgrade_modal() -> ViewModel {
    let mut vm = projects();
    vm.modal = Some(ModalVm::UpgradeProject(UpgradeProjectVm {
        project_title: "Orbital".to_string(),
        project_path_short: "…/Games/Orbital".to_string(),
        current_version: "6000.1.1f1".to_string(),
        current_architecture: "arm64".to_string(),
        target_options: vec!["6000.3.0f1".to_string()],
        target_note: None,
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

fn upgrade_modal_two_builds() -> ViewModel {
    let mut vm = upgrade_modal();
    if let Some(ModalVm::UpgradeProject(upgrade)) = &mut vm.modal {
        upgrade.target_note = Some(
            "6000.3.0f1 has arm64 and x86_64 builds installed. `unity projects upgrade` selects by version, not by build, so the CLI chooses which installed build opens the project; this dialog does not silently pick one. Review the builds on the Editors page before upgrading."
                .to_string(),
        );
    }
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

fn advisory_update() -> ViewModel {
    let mut vm = overview();
    vm.shell.notifications = vec![NotificationVm {
        id: 1,
        tone: Tone::Accent,
        label: "CLI_UPDATE_AVAILABLE".to_string(),
        message: "A new Unity CLI is available.".to_string(),
        command: Some("unity self-update".to_string()),
        can_run: false,
        state: NotificationActionState::Idle,
    }];
    vm.shell.unread = 1;
    vm
}

fn advisory_plugin_beta() -> ViewModel {
    let mut vm = editors();
    vm.shell.notifications = vec![NotificationVm {
        id: 1,
        tone: Tone::Warning,
        label: "Warning".to_string(),
        message: "This plugin is a beta release. Beta plugins can break projects.".to_string(),
        command: None,
        can_run: false,
        state: NotificationActionState::Idle,
    }];
    vm.shell.unread = 1;
    vm
}

fn advisory_unknown() -> ViewModel {
    let mut vm = overview();
    vm.shell.notifications = vec![NotificationVm {
        id: 1,
        tone: Tone::Info,
        label: "FUTURE_CODE".to_string(),
        message: "Something new happened. This CLI is newer than the app.".to_string(),
        command: None,
        can_run: false,
        state: NotificationActionState::Idle,
    }];
    vm.shell.unread = 1;
    vm
}

fn advisory_dismissed() -> ViewModel {
    let mut vm = overview();
    // A dismissal lands as a notice, which borrows the toast slot with id 0.
    vm.shell.toast = Some(ToastVm {
        id: 0,
        tone: Tone::Info,
        message: "Advisory dismissed for this CLI version.".to_string(),
    });
    vm
}

fn job_failed() -> ViewModel {
    let mut vm = overview();
    vm.shell.job = Some(JobVm {
        kind: "project-upgrade".to_string(),
        command: "unity projects upgrade /games/orbital --to 6000.3.0f1 --yes".to_string(),
        phase: JobPhase::Failed,
        progress: 0.0,
        message: Some("Upgrading project…".to_string()),
        error: Some(UiError {
            code: "NETWORK_UNREACHABLE".to_string(),
            message: "Project upgrade failed.".to_string(),
            detail: Some("The job already started a mutation, so no Retry is offered.".to_string()),
            action: None,
        }),
        logs: "! Editor 6000.3.0f1 is not installed.\n".into(),
    });
    vm
}

fn notification(id: u64, tone: Tone, label: &str, message: &str) -> NotificationVm {
    NotificationVm {
        id,
        tone,
        label: label.to_string(),
        message: message.to_string(),
        command: None,
        can_run: false,
        state: NotificationActionState::Idle,
    }
}

fn notifications_rail_unread() -> ViewModel {
    let mut vm = overview();

    let mut advisory = notification(
        3,
        Tone::Info,
        "CLI advisory",
        "Unity CLI 1.0.1 is available. Run the upgrade command to update the CLI.",
    );
    advisory.command = Some("unity upgrade --channel stable".to_string());
    advisory.can_run = true;

    vm.shell.notifications = vec![
        advisory,
        notification(
            2,
            Tone::Warning,
            "Warning",
            "The project list came from a cache. Refresh to read the projects again.",
        ),
        notification(
            1,
            Tone::Success,
            "Editor installed",
            "Unity 6000.3.0f1 finished installing.",
        ),
    ];
    vm.shell.unread = 3;
    vm.shell.rail_open = true;

    vm
}

fn notifications_rail_empty() -> ViewModel {
    let mut vm = overview();
    vm.shell.rail_open = true;
    vm
}

/// A rail long enough to scroll, so the gallery shows the scrollbar floating
/// over its reserved lane instead of the cards.
fn notifications_rail_overflow() -> ViewModel {
    let mut vm = overview();

    let mut advisory = notification(
        12,
        Tone::Info,
        "CLI advisory",
        "Unity CLI 1.0.2 is available. Run the upgrade command to update the CLI.",
    );
    advisory.command = Some("unity upgrade --channel stable".to_string());
    advisory.can_run = true;

    vm.shell.notifications = vec![
        advisory,
        notification(
            11,
            Tone::Warning,
            "Warning",
            "The project list came from a cache. Refresh to read the projects again.",
        ),
        notification(
            10,
            Tone::Success,
            "Editor installed",
            "Unity 6000.3.1f1 finished installing.",
        ),
        notification(
            9,
            Tone::Warning,
            "Warning",
            "The template registry was read from a stale snapshot. Refresh to pick up newly published templates.",
        ),
        notification(
            8,
            Tone::Success,
            "Editor installed",
            "Unity 6000.2.1f1 finished installing.",
        ),
        notification(
            7,
            Tone::Warning,
            "Warning",
            "The Editor list came from a cache. Refresh to read the Editors again.",
        ),
        notification(
            6,
            Tone::Success,
            "Project created",
            "rhythm-sampler was created from the Universal 2D template.",
        ),
        notification(
            5,
            Tone::Danger,
            "Editor install failed",
            "Unity 6000.3.2f1 could not be installed. The download stopped mid-way.",
        ),
    ];
    vm.shell.unread = 8;
    vm.shell.rail_open = true;

    vm
}

/// Three entries were present and the newest (id 4) was dismissed. Dismissal
/// removes the entry, so the honest rendered state is the remaining rail.
fn notifications_dismissed() -> ViewModel {
    let mut vm = overview();

    vm.shell.notifications = vec![
        notification(
            3,
            Tone::Warning,
            "Warning",
            "The project list came from a cache. Refresh to read the projects again.",
        ),
        notification(
            2,
            Tone::Success,
            "Editor installed",
            "Unity 6000.3.0f1 finished installing.",
        ),
    ];
    vm.shell.unread = 2;
    vm.shell.rail_open = true;

    vm
}

fn notifications_toast() -> ViewModel {
    let mut vm = overview();
    vm.shell.toast = Some(ToastVm {
        id: 7,
        tone: Tone::Info,
        message: "Unity CLI 1.0.1 is available.".to_string(),
    });
    vm
}

fn notifications_long_message() -> ViewModel {
    let mut vm = overview();

    let mut advisory = notification(
        11,
        Tone::Warning,
        "Warning",
        "The Unity CLI reported that the Editor installation cache is out of date and that the templates registry was read from a stale snapshot, so newly published templates may be missing from the list until the cache is refreshed. Refresh the cache to pick up the latest templates, or keep working with the templates already installed on this machine. No project files were changed, and no Editor installation was interrupted while the cache was being read. The next refresh runs in the background and can be cancelled at any time without losing the current view.",
    );
    advisory.command = Some(
        "unity templates refresh --channel stable --include-preview --cache-dir /home/ada/.cache/unity-rust-hub/templates --format ndjson --no-pager --no-banner --non-interactive".to_string(),
    );

    vm.shell.notifications = vec![advisory];
    vm.shell.unread = 1;
    vm.shell.rail_open = true;

    vm
}

fn notification_action_available() -> ViewModel {
    let mut vm = overview();

    let mut entry = notification(
        21,
        Tone::Warning,
        "PROJECT_BACKUP_MISSING",
        "No backup of this project exists. Create one before upgrading the Editor.",
    );
    entry.command = Some(
        "unity projects backup /home/ada/Games/Orbital --output /home/ada/Backups".to_string(),
    );
    entry.can_run = true;

    vm.shell.notifications = vec![entry];
    vm.shell.unread = 1;
    vm.shell.rail_open = true;

    vm
}

fn notification_confirming() -> ViewModel {
    let mut vm = notification_action_available();

    if let Some(entry) = vm.shell.notifications.first_mut() {
        entry.state = NotificationActionState::Confirming;
    }

    vm
}

fn notification_running() -> ViewModel {
    let mut vm = notification_action_available();

    if let Some(entry) = vm.shell.notifications.first_mut() {
        entry.state = NotificationActionState::Running;
    }

    vm
}

fn notification_failed() -> ViewModel {
    let mut vm = notification_action_available();

    if let Some(entry) = vm.shell.notifications.first_mut() {
        entry.state = NotificationActionState::Failed;
    }

    vm
}

fn notifications_job_stream_advisory() -> ViewModel {
    let mut vm = job_running();

    vm.shell.notifications = vec![notification(
        31,
        Tone::Warning,
        "INSTALLER_DISK_SPACE",
        "Only 6.4 GB is free on the install volume. The Editor install continues, but module downloads may fail.",
    )];
    vm.shell.unread = 1;
    vm.shell.rail_open = true;

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
                    code: "UNEXPECTED_OUTPUT".to_string(),
                    message: "Could not read templates.".to_string(),
                    detail: Some("The CLI returned an unexpected document.".to_string()),
                    action: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> ViewModel {
        all()
            .into_iter()
            .find(|fixture| fixture.name == name)
            .map(|fixture| fixture.view_model)
            .unwrap_or_else(|| panic!("missing fixture: {name}"))
    }

    fn only_entry(view_model: &ViewModel) -> &NotificationVm {
        view_model
            .shell
            .notifications
            .first()
            .expect("fixture should carry one notification")
    }

    #[test]
    fn rail_unread_fixture_sets_the_badge_and_entries() {
        let vm = fixture("Notifications · rail unread");

        assert!(vm.shell.rail_open);
        assert_eq!(vm.shell.unread, 3);
        assert_eq!(vm.shell.notifications.len(), 3);
        assert!(vm.shell.toast.is_none());

        assert_eq!(
            vm.shell.notifications[0].state,
            NotificationActionState::Idle
        );
        assert!(vm.shell.notifications[0].can_run);
        assert!(vm.shell.notifications[0].command.is_some());
        assert_eq!(vm.shell.notifications[1].tone, Tone::Warning);
        assert!(vm.shell.notifications[1].command.is_none());
        assert_eq!(vm.shell.notifications[2].tone, Tone::Success);
    }

    #[test]
    fn rail_empty_fixture_is_open_without_entries() {
        let vm = fixture("Notifications · rail empty");

        assert!(vm.shell.rail_open);
        assert_eq!(vm.shell.unread, 0);
        assert!(vm.shell.notifications.is_empty());
        assert!(vm.shell.toast.is_none());
    }

    #[test]
    fn dismissed_fixture_drops_the_dismissed_entry() {
        let vm = fixture("Notifications · dismissed");

        assert!(vm.shell.rail_open);
        assert_eq!(vm.shell.unread, 2);
        assert!(vm.shell.toast.is_none());

        let ids: Vec<u64> = vm
            .shell
            .notifications
            .iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(ids, vec![3, 2], "the two survivors keep their order");
        assert!(
            !ids.contains(&4),
            "the dismissed entry is gone from the rail"
        );
    }

    #[test]
    fn toast_fixture_shows_one_information_toast() {
        let vm = fixture("Notifications · toast");

        assert!(!vm.shell.rail_open);
        assert_eq!(vm.shell.unread, 0);
        assert!(vm.shell.notifications.is_empty());

        let toast = vm.shell.toast.expect("toast fixture should carry a toast");
        assert_eq!(toast.tone, Tone::Info);
        assert!(!toast.message.is_empty());
    }

    #[test]
    fn long_message_fixture_keeps_the_full_text_and_command() {
        let vm = fixture("Notifications · long message");

        assert!(vm.shell.rail_open);

        let entry = only_entry(&vm);
        assert!(
            entry.message.chars().count() >= 400,
            "message is too short: {}",
            entry.message.chars().count()
        );

        let command = entry.command.as_deref().expect("fixture needs a command");
        assert!(command.chars().count() > 80);
        assert!(
            command.contains("/home/ada/.cache/unity-rust-hub/templates"),
            "the fixture needs an unspaced token wider than the inset"
        );
    }

    #[test]
    fn action_fixtures_cover_every_state() {
        let available = fixture("Notification · action available");
        let entry = only_entry(&available);
        assert_eq!(entry.state, NotificationActionState::Idle);
        assert!(entry.can_run);
        assert!(entry.command.is_some());

        let confirming = fixture("Notification · confirming");
        assert_eq!(
            only_entry(&confirming).state,
            NotificationActionState::Confirming
        );

        let running = fixture("Notification · running");
        assert_eq!(only_entry(&running).state, NotificationActionState::Running);

        let failed = fixture("Notification · failed");
        assert_eq!(only_entry(&failed).state, NotificationActionState::Failed);
        assert!(only_entry(&failed).command.is_some());
    }

    #[test]
    fn job_stream_fixture_has_a_warning_and_a_running_job() {
        let vm = fixture("Notifications · job-stream advisory");

        assert!(vm.shell.rail_open);
        assert_eq!(vm.shell.unread, 1);
        assert_eq!(only_entry(&vm).tone, Tone::Warning);

        let job = vm.shell.job.as_ref().expect("fixture needs a job");
        assert_eq!(job.phase, JobPhase::Running);
    }
}
