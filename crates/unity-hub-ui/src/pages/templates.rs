use iced::widget::{column, container, pick_list, row, space};
use iced::{Center, Element, Fill, FillPortion};

use unity_hub_model::{
    TemplateAuthoringMode, TemplateDetailVm, TemplateKindFilter, TemplateRowVm, TemplatesVm, Tone,
    UiAction,
};

use crate::components::{
    badge, button, caption, checkbox, divided, empty, error_text, hint, icon_button, list_row,
    loading, mono, page_header, refresh_button, ButtonKind, PICK_LIST_PADDING,
};
use crate::fonts;
use crate::icons::{self, icon};
use crate::theme;
use crate::tokens::{font, spacing};
use crate::ui::{self, text};

pub fn view(vm: &TemplatesVm) -> Element<'_, UiAction> {
    let actions = row![
        icon_button(icons::PACKAGE, "Create template", ButtonKind::Secondary).on_press(
            UiAction::OpenTemplateAuthoring(TemplateAuthoringMode::Create)
        ),
        icon_button(icons::DOWNLOAD, "Pack template", ButtonKind::Secondary)
            .on_press(UiAction::OpenTemplateAuthoring(TemplateAuthoringMode::Pack)),
        refresh_button(),
    ]
    .spacing(8);

    column![
        page_header(
            "Templates",
            "Browse the templates for an installed Editor, then create or pack your own.",
            Some(actions.into()),
        ),
        toolbar(vm),
        row![
            container(list_card(vm)).width(FillPortion(2)),
            container(detail_card(vm)).width(FillPortion(1)),
        ]
        .spacing(spacing::SECTION)
        .align_y(iced::Alignment::Start),
    ]
    .spacing(spacing::SECTION)
    .into()
}

fn toolbar(vm: &TemplatesVm) -> Element<'_, UiAction> {
    let editors = vm.editors.data().cloned().unwrap_or_default();
    let editor_picker: Element<'_, UiAction> = if editors.is_empty() {
        text("No installed Editor")
            .size(font::BODY)
            .style(theme::faint_text)
            .into()
    } else {
        pick_list(
            editors,
            vm.selected_editor.clone(),
            UiAction::TemplatesEditorChanged,
        )
        .text_size(ui::font_size(font::BODY))
        .font(fonts::regular())
        .padding(PICK_LIST_PADDING)
        .handle(icons::pick_list_handle(font::ICON_SMALL))
        .style(theme::pick_list)
        .menu_style(theme::menu)
        .width(220)
        .into()
    };

    let filters = crate::components::segmented(
        TemplateKindFilter::ALL
            .iter()
            .map(|filter| {
                (
                    filter.label().to_string(),
                    *filter == vm.filter,
                    UiAction::TemplateFilterChanged(*filter),
                )
            })
            .collect(),
    );

    row![
        caption("Editor"),
        editor_picker,
        filters,
        space().width(Fill),
        checkbox(
            "Installed only",
            vm.installed_only,
            UiAction::TemplateInstalledOnlyToggled,
        ),
    ]
    .spacing(12)
    .align_y(Center)
    .into()
}

fn list_card(vm: &TemplatesVm) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = if vm.editors.is_loading() && vm.editors.data().is_none() {
        loading("Reading installed Editors…")
    } else if let Some(error) = vm.editors.error() {
        error_text(error)
    } else if vm.editors.data().is_some_and(Vec::is_empty) {
        empty(
            "No Unity Editor installed",
            "Templates come from an Editor version. Install one on the Editors page first.",
        )
    } else if let Some(error) = vm.list.error() {
        column![
            error_text(error),
            button("Retry", ButtonKind::Secondary).on_press(UiAction::Refresh),
        ]
        .spacing(12)
        .into()
    } else if let Some(templates) = vm.list.data() {
        if templates.is_empty() {
            empty(
                "No templates in this filter",
                "Choose All, or clear the installed-only filter.",
            )
        } else {
            divided(
                templates
                    .iter()
                    .map(template_row)
                    .collect::<Vec<Element<'_, UiAction>>>(),
            )
            .into()
        }
    } else if vm.list.is_loading() {
        loading("Reading templates…")
    } else {
        space().height(0).into()
    };

    container(body)
        .padding([0.0, spacing::CARD])
        .width(Fill)
        .style(theme::card)
        .into()
}

fn template_row(template: &TemplateRowVm) -> Element<'_, UiAction> {
    let title = row![
        text(template.display_name.as_str())
            .size(font::BODY_LARGE)
            .font(fonts::semibold()),
        badge(template.status.clone(), template.status_tone),
    ]
    .spacing(8)
    .align_y(Center);

    let mut meta = row![mono(template.name.as_str())]
        .spacing(10)
        .align_y(Center);
    if let Some(pipeline) = &template.pipeline {
        meta = meta.push(badge(pipeline.clone(), Tone::Highlight));
    }
    if !template.version.is_empty() {
        meta = meta.push(
            text(format!("v{}", template.version))
                .size(font::SMALL)
                .style(theme::faint_text),
        );
    }

    let content = row![
        icon(icons::PACKAGE, font::ICON).style(theme::faint_text),
        column![title, meta].spacing(4).width(Fill),
        text(template.kind.to_uppercase())
            .size(font::CAPTION)
            .style(theme::faint_text),
        icon(icons::CHEVRON_RIGHT, font::ICON_SMALL).style(theme::faint_text),
    ]
    .spacing(12)
    .align_y(Center);

    iced::widget::button(list_row(content))
        .width(Fill)
        .padding(0)
        .style(theme::button_ghost)
        .on_press(UiAction::OpenTemplateDetail(template.name.clone()))
        .into()
}

fn detail_card(vm: &TemplatesVm) -> Element<'_, UiAction> {
    let body: Element<'_, UiAction> = if vm.detail_loading {
        loading("Reading template details…")
    } else if let Some(error) = &vm.detail_error {
        error_text(error)
    } else if let Some(detail) = &vm.detail {
        detail_body(detail)
    } else {
        hint("Select a template to inspect its description, pipeline, and packages.")
    };

    container(body)
        .padding([0.0, spacing::CARD])
        .width(Fill)
        .style(theme::card)
        .into()
}

fn detail_body(detail: &TemplateDetailVm) -> Element<'_, UiAction> {
    let mut body = column![
        row![
            text(detail.display_name.as_str())
                .size(font::CARD_TITLE)
                .font(fonts::semibold())
                .width(Fill),
            badge(
                if detail.installed {
                    "installed"
                } else {
                    "not installed"
                },
                if detail.installed {
                    Tone::Success
                } else {
                    Tone::Neutral
                },
            ),
        ]
        .spacing(8)
        .align_y(Center),
        mono(detail.name.as_str()),
    ]
    .spacing(6);

    if !detail.description.trim().is_empty() {
        body = body.push(
            text(detail.description.as_str())
                .size(font::BODY)
                .line_height(1.55)
                .style(theme::muted_text),
        );
    }

    body = body.push(
        row![
            fact("Version", detail.version.as_str()),
            fact("Kind", detail.kind.as_str()),
        ]
        .spacing(20),
    );

    body = body.push(
        row![
            fact(
                "Render pipeline",
                detail.render_pipeline.as_deref().unwrap_or("Not reported"),
            ),
            fact("Status", detail.status.as_str()),
        ]
        .spacing(20),
    );

    if !detail.previous_versions.is_empty() {
        body = body.push(caption("Previous versions"));
        body = body.push(
            text(detail.previous_versions.join(" · "))
                .size(font::MONO)
                .font(fonts::mono())
                .style(theme::muted_text),
        );
    }

    if !detail.packages.is_empty() {
        body = body.push(caption(format!("Packages · {}", detail.packages.len())));
        body = body.push(
            container(crate::components::scroll(
                column(
                    detail
                        .packages
                        .iter()
                        .map(|package| {
                            text(package.as_str())
                                .size(font::MONO)
                                .font(fonts::mono())
                                .style(theme::muted_text)
                                .into()
                        })
                        .collect::<Vec<Element<'_, UiAction>>>(),
                )
                .spacing(4),
            ))
            .max_height(180)
            .padding([10.0, 12.0])
            .width(Fill)
            .style(theme::inset),
        );
    }

    body.into()
}

fn fact<'a>(label: &'a str, value: &'a str) -> Element<'a, UiAction> {
    column![
        caption(label),
        text(value).size(font::BODY).style(theme::primary_text),
    ]
    .spacing(3)
    .width(Fill)
    .into()
}
