use iced::widget::{button, column, row, text, text_input};
use iced::{Element, Fill};

use crate::app::{badge, App, Message};
use crate::theme;
use crate::util::{error_text, format_epoch_millis, short_path};
use crate::views::{card, cell, empty, header_cell, page_header, row_card, row_header};

pub fn view(app: &App) -> Element<'_, Message> {
    let mut page = column![page_header(
        "HUB REGISTRY",
        "Projects",
        "Every project the Unity CLI knows about, its Editor version, and a guarded upgrade path.",
    )]
    .spacing(16);

    page = page.push(projects_card(app));
    page.into()
}

fn projects_card(app: &App) -> Element<'_, Message> {
    let mut body = column![].spacing(10);

    let filtered = app.filtered_projects();
    let total = app.projects.data.as_ref().map(Vec::len).unwrap_or(0);

    body = body.push(
        row![
            text_input("Filter by name, path, or version", &app.project_search)
                .on_input(Message::ProjectSearchChanged)
                .padding(8)
                .size(11)
                .width(Fill),
            text(format!("{} of {}", filtered.len(), total))
                .size(9)
                .style(theme::faint_text),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    );

    if app.projects.loading && app.projects.data.is_none() {
        body = body.push(text("Reading projects…").size(11).style(theme::muted_text));
    } else if let Some(error) = &app.projects.error {
        body = body.push(text(error_text(error)).size(10).style(theme::danger_text));
    } else if let Some(projects) = &app.projects.data {
        if projects.is_empty() {
            body = body.push(empty(
                "No Unity projects registered",
                "Add a project folder to the Unity Hub registry, then refresh.",
            ));
        } else {
            if filtered.is_empty() {
                body = body.push(empty("No matching projects", "Try a shorter search."));
            } else {
                let installed = app.installed_versions();

                body = body.push(row_header(vec![
                    header_cell("Project", 4),
                    header_cell("Version", 2),
                    header_cell("Pipeline", 2),
                    header_cell("Target", 2),
                    header_cell("Path", 3),
                    header_cell("Modified", 2),
                    header_cell("", 2),
                ]));

                for project in filtered {
                    let mut badges = Vec::new();
                    if project.cloud_enabled {
                        badges.push("Unity Cloud");
                    }
                    if let Some(vcs) = &project.vcs_provider {
                        badges.push(vcs.as_str());
                    }
                    if !installed.is_empty() && !installed.contains(&project.version) {
                        badges.push("Editor not installed");
                    }
                    if project.is_favorite {
                        badges.push("favorite");
                    }
                    let badges = if badges.is_empty() {
                        "local project".to_string()
                    } else {
                        badges.join(" · ")
                    };

                    let pipeline: Element<'_, Message> = project
                        .render_pipeline
                        .clone()
                        .map(|pipeline| badge(pipeline, theme::Tone::Violet))
                        .unwrap_or_else(|| text("—").size(10).style(theme::faint_text).into());

                    body = body.push(row_card(vec![
                        cell(
                            column![
                                text(project.title.clone()).size(11),
                                text(badges).size(9).style(theme::faint_text),
                            ]
                            .spacing(1),
                            4,
                        ),
                        cell(text(project.version.clone()).size(10), 2),
                        cell(pipeline, 2),
                        cell(
                            text(
                                project
                                    .build_target
                                    .clone()
                                    .unwrap_or_else(|| "—".to_string()),
                            )
                            .size(9)
                            .style(theme::muted_text),
                            2,
                        ),
                        cell(
                            text(short_path(&project.path, 42))
                                .size(9)
                                .style(theme::faint_text),
                            3,
                        ),
                        cell(
                            text(format_epoch_millis(project.last_modified))
                                .size(9)
                                .style(theme::faint_text),
                            2,
                        ),
                        cell(
                            button(text("Change version").size(9))
                                .on_press(Message::OpenUpgrade(project.clone()))
                                .padding([3.0, 7.0])
                                .style(button::secondary),
                            2,
                        ),
                    ]));
                }
            }
        }
    }

    card("Registered projects", body.into())
}
