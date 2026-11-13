use iced::widget::{column, container, row, space};
use iced::{Center, Element, Fill, FillPortion, Length};

use unity_hub_model::{ProjectRowVm, ProjectsVm, UiAction};

use crate::components::{
    self, badge, button, caption, cell, divided, empty, error_text, icon_button, initial_tile,
    list_row, loading, mono, page_header, refresh_button, search_input, ButtonKind,
};
use crate::fonts;
use crate::icons;
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

/// Room for the "Change version" and "Open" buttons at 100% text; it grows
/// with the text-size preference.
const ACTIONS_WIDTH: f32 = 236.0;

pub fn view(vm: &ProjectsVm) -> Element<'_, UiAction> {
    column![
        page_header(
            "Projects",
            "Every project in the Hub registry, with a guarded upgrade path.",
            Some(refresh_button()),
        ),
        row![
            search_input("Filter by name, path, or version", &vm.search)
                .on_input(UiAction::ProjectSearchChanged)
                .width(Fill),
            text(format!(
                "{} of {} projects",
                vm.visible_count, vm.total_count
            ))
            .size(font::BODY)
            .style(theme::faint_text),
        ]
        .spacing(16)
        .align_y(Center),
        projects_card(vm),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn columns<'a>(
    project: Element<'a, UiAction>,
    version: Element<'a, UiAction>,
    pipeline: Element<'a, UiAction>,
    target: Element<'a, UiAction>,
    modified: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    row![
        cell(project, FillPortion(5)),
        cell(version, FillPortion(2)),
        cell(pipeline, FillPortion(2)),
        cell(target, FillPortion(2)),
        cell(modified, FillPortion(2)),
        cell(actions, Length::Fixed(ui::scaled_pixels(ACTIONS_WIDTH))),
    ]
    .spacing(16)
    .align_y(Center)
    .into()
}

fn projects_card(vm: &ProjectsVm) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = if let Some(error) = vm.projects.error() {
        container(error_text(error)).padding([20.0, 0.0]).into()
    } else if let Some(projects) = vm.projects.data() {
        if vm.total_count == 0 {
            empty(
                "No Unity projects registered",
                "Add a project folder to the Unity Hub registry, then refresh.",
            )
        } else if projects.is_empty() {
            empty("No matching projects", "Try a shorter search.")
        } else {
            let header = list_row(columns(
                caption("Project"),
                caption("Editor"),
                caption("Pipeline"),
                caption("Target"),
                caption("Modified"),
                space().width(0).into(),
            ));

            let mut rows = vec![header];
            rows.extend(projects.iter().map(project_row));
            divided(rows).into()
        }
    } else if vm.projects.is_loading() {
        container(loading("Reading projects…"))
            .padding([20.0, 0.0])
            .into()
    } else {
        space().height(0).into()
    };

    container(body)
        .padding([0.0, spacing::CARD])
        .width(Fill)
        .style(theme::card)
        .into()
}

fn project_row(project: &ProjectRowVm) -> Element<'_, UiAction> {
    let mut details = column![
        text(project.title.as_str())
            .size(font::BODY_LARGE)
            .font(fonts::semibold()),
        mono(project.short_path.as_str()),
    ]
    .spacing(3);
    if !project.badges.is_empty() {
        details = details.push(
            container(
                row(project
                    .badges
                    .iter()
                    .map(|item| badge(item.label.clone(), item.tone))
                    .collect::<Vec<_>>())
                .spacing(6),
            )
            .padding([4.0, 0.0]),
        );
    }

    let name = row![initial_tile(&project.title, 40.0), details]
        .spacing(14)
        .align_y(Center);

    let pipeline: Element<'_, UiAction> = project
        .pipeline
        .as_ref()
        .map(|pipeline| badge(pipeline.label.clone(), pipeline.tone))
        .unwrap_or_else(|| text("—").style(theme::faint_text).into());

    let target: Element<'_, UiAction> = if project.build_target.is_empty() {
        text("—").style(theme::faint_text).into()
    } else {
        text(project.build_target.as_str())
            .size(font::BODY)
            .style(theme::muted_text)
            .into()
    };

    list_row(columns(
        name.into(),
        text(project.version.as_str())
            .size(font::MONO)
            .font(fonts::mono())
            .into(),
        pipeline,
        target,
        text(project.modified.as_str())
            .size(font::BODY)
            .style(theme::faint_text)
            .into(),
        row![
            space().width(Fill),
            button("Change version", ButtonKind::Secondary)
                .on_press(UiAction::OpenProjectUpgrade(project.path.clone()))
                .padding(components::COMPACT),
            icon_button(icons::PLAY, "Open", ButtonKind::Primary)
                .on_press(UiAction::OpenProject(project.path.clone()))
                .padding(components::COMPACT),
        ]
        .spacing(8)
        .align_y(Center)
        .into(),
    ))
}
