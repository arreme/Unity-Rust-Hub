//! Live smoke tests for the Unity CLI adapter.
//!
//! These are ignored by default because they require a working `unity`
//! installation (and some need network access). Run them with:
//!
//! ```text
//! cargo test -p unity-core --test live -- --ignored --nocapture
//! ```
//!
//! Written against Unity CLI 1.0.1. No test installs, creates, packs, upgrades,
//! removes or self-updates anything; one test starts the auth broker.

use unity_core::changelog::{
    parse_changelog, ChangelogProvider, ChangelogRequest, CliChangelogProvider,
};
use unity_core::models::{CreateProjectOptions, TemplateFilter};
use unity_core::surface::REQUIRED_COMMAND_PATHS;
use unity_core::UnityCli;

use std::collections::BTreeSet;

use serde_json::Value;

#[test]
#[ignore = "requires the Unity CLI"]
fn reads_real_cli_data() {
    let cli = UnityCli::new();

    let location = cli.locate().expect("Unity CLI should be locatable");
    println!(
        "resolved: {} ({})",
        location.path.display(),
        location.source
    );

    let version = cli.version().expect("unity version");
    println!("version: {} ({})", version.version, version.channel);
    assert!(!version.version.is_empty());

    let install_path = cli.install_path().expect("unity install-path");
    println!("install path: {install_path}");

    cli.ensure_auth_broker()
        .expect("unity auth broker ensure should be accepted");
    println!("auth broker: ensured");

    let editors = cli.editors_installed().expect("unity editors --installed");
    println!("installed editors: {}", editors.len());
    for editor in &editors {
        println!(
            "  {} {} modules={:?}",
            editor.version, editor.architecture, editor.module_names
        );
    }
    assert!(
        !editors.is_empty(),
        "expected at least one installed Editor"
    );

    let first_version = editors[0].version.clone();
    let first_architecture = editors[0].architecture.clone();
    let modules = cli
        .modules(&first_version, Some(&first_architecture))
        .expect("unity modules");
    println!(
        "modules for {first_version} ({first_architecture}): {}",
        modules.len()
    );
    assert!(!modules.is_empty());

    let info = cli.editor_info(&first_version).expect("unity editors info");
    println!(
        "info for {}: stream={} notes={:?}",
        info.version, info.stream, info.release_notes_url
    );

    let releases = cli.releases(3, 0, None).expect("unity releases");
    println!(
        "releases: {:?}",
        releases.iter().map(|r| &r.version).collect::<Vec<_>>()
    );
    assert!(!releases.is_empty());

    let projects = cli.projects().expect("unity projects");
    println!("projects: {}", projects.len());
    for project in &projects {
        println!("  {} -> {}", project.title, project.version);
    }

    let running = cli.editors_running().expect("unity editors running");
    println!("running editors: {}", running.count);

    let templates = cli
        .templates(&first_version, TemplateFilter::Core, false)
        .expect("unity templates --brief");
    println!("templates for {first_version}: {}", templates.len());
    assert!(!templates.is_empty());
    let first_template = templates
        .iter()
        .find(|template| template.name.contains("urp-blank"))
        .or_else(|| templates.first())
        .expect("a template");
    let template_info = cli
        .template_info(&first_version, &first_template.name)
        .expect("unity templates info");
    println!(
        "template info: {} pipeline={:?} status={}",
        template_info.name,
        template_info.render_pipeline_label(),
        template_info.status
    );

    let status = cli.status_instances().expect("unity status");
    println!("status instances: {}", status.len());

    let auth = cli.auth_status().expect("unity auth status");
    println!(
        "auth: logged_in={} state={:?}",
        auth.logged_in, auth.session_state
    );

    let license = cli.license_status().expect("unity license status");
    println!(
        "license: active={} count={}",
        license.active,
        license.licenses.len()
    );

    let update = cli.self_update_check().expect("unity self-update --check");
    println!(
        "update: {} -> {} available={}",
        update.current_version, update.latest_version, update.update_available
    );

    let provider = CliChangelogProvider::new(cli.clone());
    let request = ChangelogRequest::default();
    let raw = provider.fetch(&request).expect("unity changelog");
    println!("changelog provider: {}", provider.source_id());
    let parsed = parse_changelog(&raw.changelog);
    println!(
        "changelog {}: {} sections, {} entries, {} possible breaking",
        raw.version,
        parsed.sections.len(),
        parsed.entry_count(),
        parsed.breaking_changes.len()
    );
    assert!(!parsed.sections.is_empty());

    let mut by_section = std::collections::BTreeMap::new();
    for change in &parsed.breaking_changes {
        *by_section.entry(change.section.clone()).or_insert(0usize) += 1;
    }
    println!("breaking by section: {by_section:?}");
    for change in parsed.breaking_changes.iter().take(25) {
        println!("  [{}] {}", change.reason, change.text);
    }
}

/// `doctor` and `env` describe the machine the CLI runs on. Both must parse
/// and report success, so a diagnostic taken right after a CLI bump is
/// trustworthy.
#[test]
#[ignore = "requires the Unity CLI"]
fn doctor_and_env_return_success_envelopes() {
    let cli = UnityCli::new();

    for command in ["doctor", "env"] {
        let executed = cli
            .run_args(&[command.to_string()], "json")
            .unwrap_or_else(|error| panic!("`unity {command} --format json` failed: {error}"));
        assert!(
            executed.envelope.success,
            "`unity {command}` returned success: false"
        );
        let data = executed
            .envelope
            .data
            .as_ref()
            .unwrap_or_else(|| panic!("`unity {command}` should return data"));
        println!(
            "{command}: success (cliVersion {})",
            data.get("cliVersion")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        );
    }
}

/// Templates are read per installed Editor. A machine with no Editor cannot
/// answer the read, so print a note and pass instead of failing.
#[test]
#[ignore = "requires the Unity CLI"]
fn templates_for_the_first_installed_editor() {
    let cli = UnityCli::new();
    let editors = cli.editors_installed().expect("unity editors --installed");

    let Some(editor) = editors.first() else {
        println!("no installed Editor; skipping the templates read");
        return;
    };

    let templates = cli
        .templates(&editor.version, TemplateFilter::All, false)
        .expect("unity templates --brief");
    println!(
        "templates for {}: {} (first: {})",
        editor.version,
        templates.len(),
        templates
            .first()
            .map(|template| template.name.as_str())
            .unwrap_or("-")
    );
}

/// `unity status` exits 12 when no Editor is connected; the adapter turns that
/// into an empty list, so `Ok` is the whole contract. An empty list is normal
/// on a machine with no Editor running.
#[test]
#[ignore = "requires the Unity CLI"]
fn status_returns_ok_with_or_without_running_editors() {
    let cli = UnityCli::new();
    let instances = cli.status_instances().expect("unity status");
    println!("status instances: {}", instances.len());
}

/// `projects create` is never spawned here: only the argv the wizard would run
/// is checked. The spec is built from a synthetic output path and nothing is
/// written to disk.
#[test]
#[ignore = "requires the Unity CLI"]
fn projects_create_spec_passes_the_non_interactive_flags() {
    let cli = UnityCli::new();
    let options = CreateProjectOptions {
        name: "NotificationSmoke".to_string(),
        output_dir: std::env::temp_dir()
            .join("unity-rust-hub-live-smoke")
            .display()
            .to_string(),
        editor_version: "6000.3.15f1".to_string(),
        template: "com.unity.template.urp-blank".to_string(),
        architecture: "x86_64".to_string(),
        open: false,
        bare: false,
    };

    let spec = cli
        .create_project_spec(&options)
        .expect("`unity projects create` spec");
    println!("projects create: {}", spec.display());

    assert_eq!(spec.args.first().map(String::as_str), Some("projects"));
    assert_eq!(spec.args.get(1).map(String::as_str), Some("create"));

    for flag in [
        "--no-cloud",
        "--format",
        "--non-interactive",
        "--no-pager",
        "--no-banner",
    ] {
        assert!(
            spec.args.iter().any(|arg| arg == flag),
            "argv is missing `{flag}`: {:?}",
            spec.args
        );
    }

    assert!(
        spec.args
            .windows(2)
            .any(|pair| pair[0] == "--format" && pair[1] == "ndjson"),
        "argv should request ndjson: {:?}",
        spec.args
    );
}

#[test]
#[ignore = "requires the Unity CLI"]
fn app_command_surface_is_still_exposed() {
    let cli = UnityCli::new();
    let executed = cli
        .run_args(&["commands".to_string()], "json")
        .expect("unity commands --format json");
    let data = executed
        .envelope
        .data
        .expect("`unity commands` should return data");
    let commands = data
        .get("commands")
        .and_then(Value::as_array)
        .expect("`data.commands` should be an array");

    let mut paths = BTreeSet::new();
    collect_command_paths(commands, &mut paths);

    let missing: Vec<&str> = REQUIRED_COMMAND_PATHS
        .iter()
        .copied()
        .filter(|path| !paths.contains(*path))
        .collect();
    assert!(
        missing.is_empty(),
        "the CLI no longer exposes commands the app builds: {missing:?}"
    );
    println!(
        "checked {} command paths against `unity commands`",
        REQUIRED_COMMAND_PATHS.len()
    );
}

fn collect_command_paths(nodes: &[Value], paths: &mut BTreeSet<String>) {
    for node in nodes {
        if let Some(path) = node.get("path").and_then(Value::as_str) {
            paths.insert(path.to_string());
        }
        if let Some(children) = node.get("subcommands").and_then(Value::as_array) {
            collect_command_paths(children, paths);
        }
    }
}
