use iced::widget::{column, container, row, space};
use iced::{Center, Element, Fill, Length};

use unity_hub_model::{ProjectRowVm, ProjectsVm, UiAction};

use crate::components::{
    self, badge, button, caption, cell, divided, empty, error_text, icon_button, icon_only,
    initial_tile, list_row, loading, mono, page_header, refresh_button, scroll_horizontal,
    search_input, ButtonKind,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

/// Room for the favorite, upgrade, open, and remove controls at 100% text; the
/// text part grows with the text-size preference.
const ACTIONS_TEXT_WIDTH: f32 = 160.0;
const ACTIONS_FIXED_WIDTH: f32 = 110.0;
const PROJECT_MIN_WIDTH: f32 = 232.0;
const VERSION_WIDTH: f32 = 90.0;
const PIPELINE_WIDTH: f32 = 72.0;
const TARGET_WIDTH: f32 = 85.0;
const MODIFIED_WIDTH: f32 = 100.0;
const COLUMN_SPACING: f32 = 12.0;
const COLUMN_COUNT: f32 = 6.0;

fn scaled(width: f32) -> f32 {
    ui::scaled_pixels(width)
}

fn actions_width() -> f32 {
    scaled(ACTIONS_TEXT_WIDTH) + ACTIONS_FIXED_WIDTH
}

/// Width of every column except the flexible project column.
fn fixed_widths() -> f32 {
    scaled(VERSION_WIDTH + PIPELINE_WIDTH + TARGET_WIDTH + MODIFIED_WIDTH)
        + actions_width()
        + COLUMN_SPACING * (COLUMN_COUNT - 1.0)
}

fn min_table_width() -> f32 {
    scaled(PROJECT_MIN_WIDTH) + fixed_widths()
}

pub fn view(vm: &ProjectsVm) -> Element<'_, UiAction> {
    let actions = row![
        icon_button(icons::PLUS, "New project", ButtonKind::Primary)
            .on_press(UiAction::OpenNewProject),
        icon_button(icons::FOLDERS, "Add existing", ButtonKind::Secondary)
            .on_press(UiAction::AddProject),
        refresh_button(),
    ]
    .spacing(8);

    column![
        page_header(
            "Projects",
            "Every project in the Hub registry, with a guarded upgrade path.",
            Some(actions.into()),
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
    project_width: f32,
    project: Element<'a, UiAction>,
    version: Element<'a, UiAction>,
    pipeline: Element<'a, UiAction>,
    target: Element<'a, UiAction>,
    modified: Element<'a, UiAction>,
    actions: Element<'a, UiAction>,
) -> Element<'a, UiAction> {
    row![
        cell(project, Length::Fixed(project_width)),
        cell(version, Length::Fixed(scaled(VERSION_WIDTH))),
        cell(pipeline, Length::Fixed(scaled(PIPELINE_WIDTH))),
        cell(target, Length::Fixed(scaled(TARGET_WIDTH))),
        cell(modified, Length::Fixed(scaled(MODIFIED_WIDTH))),
        cell(actions, Length::Fixed(actions_width())),
    ]
    .spacing(COLUMN_SPACING)
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
                "Choose New project to create one, or Add existing to register a folder.",
            )
        } else if projects.is_empty() {
            empty("No matching projects", "Try a shorter search.")
        } else {
            let min_width = min_table_width();

            iced::widget::responsive(move |size| {
                let table_width = if size.width.is_finite() {
                    size.width.max(min_width)
                } else {
                    min_width
                };
                let project_width = table_width - fixed_widths();

                let header = list_row(columns(
                    project_width,
                    caption("Project"),
                    caption("Editor"),
                    caption("Pipeline"),
                    caption("Target"),
                    caption("Modified"),
                    space().width(0).into(),
                ));

                let mut rows = vec![header];
                rows.extend(
                    projects
                        .iter()
                        .map(|project| project_row(project, project_width)),
                );

                let table = divided(rows).width(Length::Fixed(table_width));
                if table_width > size.width {
                    scroll_horizontal(table).into()
                } else {
                    table.into()
                }
            })
            .height(Length::Shrink)
            .into()
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

fn project_row(project: &ProjectRowVm, project_width: f32) -> Element<'_, UiAction> {
    let favorite = iced::widget::button(icon(icons::STAR, font::ICON_SMALL))
        .on_press(UiAction::ToggleProjectFavorite(
            project.path.clone(),
            !project.is_favorite,
        ))
        .padding([2.0, 4.0])
        .style(theme::button_favorite(project.is_favorite));

    let title = row![
        text(project.title.as_str())
            .size(font::BODY_LARGE)
            .font(fonts::semibold()),
        favorite,
    ]
    .spacing(4)
    .align_y(Center);

    let mut details = column![title, mono(project.short_path.as_str())]
        .spacing(3)
        .width(Fill);
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
            .padding([4.0, 0.0])
            .width(Fill)
            .clip(true),
        );
    }

    let name = row![initial_tile(&project.title, 40.0), details]
        .spacing(14)
        .align_y(Center)
        .width(Fill);

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
            .width(Fill)
            .wrapping(iced::widget::text::Wrapping::Glyph)
            .style(theme::muted_text)
            .into()
    };

    list_row(columns(
        project_width,
        name.into(),
        text(project.version.as_str())
            .size(font::MONO)
            .font(fonts::mono())
            .width(Fill)
            .wrapping(iced::widget::text::Wrapping::Glyph)
            .into(),
        pipeline,
        target,
        text(project.modified.as_str())
            .size(font::BODY)
            .width(Fill)
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
            icon_only(icons::TRASH, ButtonKind::Ghost)
                .on_press(UiAction::OpenProjectRemove(project.path.clone()))
                .style(theme::button_danger_ghost),
        ]
        .spacing(8)
        .align_y(Center)
        .into(),
    ))
}
