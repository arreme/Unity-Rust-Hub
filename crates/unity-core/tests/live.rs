//! Live smoke tests for the Unity CLI adapter.
//!
//! These are ignored by default because they require a working `unity`
//! installation (and some need network access). Run them with:
//!
//! ```text
//! cargo test -p unity-core --test live -- --ignored --nocapture
//! ```

use unity_core::changelog::{
    parse_changelog, ChangelogProvider, ChangelogRequest, CliChangelogProvider,
};
use unity_core::models::TemplateFilter;
use unity_core::UnityCli;

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
    let modules = cli.modules(&first_version).expect("unity modules");
    println!("modules for {first_version}: {}", modules.len());
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
