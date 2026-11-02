use iced::widget::{button, column, row, text_input};
use iced::{Element, Fill};

use unity_hub_model::{ProjectRowVm, ProjectsVm, UiAction};

use crate::components::{badge, card, cell, empty, header_cell, page_header, row_card, row_header};
use crate::theme;
use crate::ui::{self, text};

pub fn view(vm: &ProjectsVm) -> Element<'_, UiAction> {
    let mut page = column![page_header(
        "HUB REGISTRY",
        "Projects",
        "Every project the Unity CLI knows about, its Editor version, and a guarded upgrade path.",
    )]
    .spacing(16);

    page = page.push(projects_card(vm));
    page.into()
}

fn projects_card(vm: &ProjectsVm) -> Element<'_, UiAction> {
    let mut body = column![].spacing(10);

    body = body.push(
        row![
            text_input("Filter by name, path, or version", &vm.search)
                .on_input(UiAction::ProjectSearchChanged)
                .padding(8)
                .size(ui::font_size(11.0))
                .width(Fill),
            text(format!("{} of {}", vm.visible_count, vm.total_count))
                .size(9)
                .style(theme::faint_text),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    );

    if let Some(error) = vm.projects.error() {
        body = body.push(text(error.text()).size(10).style(theme::danger_text));
    } else if let Some(projects) = vm.projects.data() {
        if vm.total_count == 0 {
            body = body.push(empty(
                "No Unity projects registered",
                "Add a project folder to the Unity Hub registry, then refresh.",
            ));
        } else if projects.is_empty() {
            body = body.push(empty("No matching projects", "Try a shorter search."));
        } else {
            body = body.push(row_header(vec![
                header_cell("Project", 4),
                header_cell("Version", 2),
                header_cell("Pipeline", 2),
                header_cell("Target", 2),
                header_cell("Path", 2),
                header_cell("Modified", 2),
                header_cell("", 3),
            ]));

            for project in projects {
                body = body.push(project_row(project));
            }
        }
    } else if vm.projects.is_loading() {
        body = body.push(text("Reading projects…").size(11).style(theme::muted_text));
    }

    card("Registered projects", body.into())
}

fn project_row(project: &ProjectRowVm) -> Element<'_, UiAction> {
    let badge_text = project
        .badges
        .iter()
        .map(|badge| badge.label.as_str())
        .collect::<Vec<_>>()
        .join(" · ");

    let pipeline: Element<'_, UiAction> = project
        .pipeline
        .as_ref()
        .map(|pipeline| badge(pipeline.label.clone(), pipeline.tone))
        .unwrap_or_else(|| text("—").size(10).style(theme::faint_text).into());

    row_card(vec![
        cell(
            column![
                text(project.title.as_str()).size(11),
                text(badge_text).size(9).style(theme::faint_text),
            ]
            .spacing(1),
            4,
        ),
        cell(text(project.version.as_str()).size(10), 2),
        cell(pipeline, 2),
        cell(
            text(project.build_target.as_str())
                .size(9)
                .style(theme::muted_text),
            2,
        ),
        cell(
            text(project.short_path.as_str())
                .size(9)
                .style(theme::faint_text),
            2,
        ),
        cell(
            text(project.modified.as_str())
                .size(9)
                .style(theme::faint_text),
            2,
        ),
        cell(
            row![
                button(text("Open").size(9))
                    .on_press(UiAction::OpenProject(project.path.clone()))
                    .padding([3.0, 7.0])
                    .style(button::primary),
                button(text("Change version").size(9))
                    .on_press(UiAction::OpenProjectUpgrade(project.path.clone()))
                    .padding([3.0, 7.0])
                    .style(button::secondary),
            ]
            .spacing(4),
            3,
        ),
    ])
}
