use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{column, container, pick_list, row, space};
use iced::window::Screenshot;
use iced::{Element, Fill, Task, Theme};

use unity_hub_model::UiAction;
use unity_hub_ui::fixtures::{self, Fixture};

fn main() -> iced::Result {
    // `GALLERY_HEIGHT=<px>` opens a taller window, e.g. to capture job cards
    // below the fold.
    let size = std::env::var("GALLERY_HEIGHT")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .map_or(unity_hub_ui::WINDOW_SIZE, |height| {
            iced::Size::new(unity_hub_ui::WINDOW_SIZE.width, height)
        });

    let mut app = iced::application(Gallery::boot, Gallery::update, Gallery::view);
    for font in unity_hub_ui::fonts::ALL {
        app = app.font(font);
    }

    app.default_font(unity_hub_ui::fonts::SANS)
        .title("Unity Rust Hub · design gallery")
        .theme(Gallery::theme)
        .window(iced::window::Settings {
            size,
            min_size: Some(unity_hub_ui::MIN_WINDOW_SIZE),
            ..Default::default()
        })
        .centered()
        .run()
}

struct Gallery {
    fixtures: Vec<Fixture>,
    selected: usize,
    /// Set by `GALLERY_SCREENSHOTS=<dir>`: capture every fixture, then exit.
    capture_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
enum Message {
    Select(&'static str),
    Ui(UiAction),
    Capture,
    Captured(Screenshot),
}

/// Lets the window finish laying out and loading fonts before a capture.
fn settle() -> Task<Message> {
    Task::perform(
        async { std::thread::sleep(Duration::from_millis(700)) },
        |()| Message::Capture,
    )
}

impl Gallery {
    fn boot() -> (Self, Task<Message>) {
        let mut fixtures = fixtures::all();
        // `GALLERY_FONT=Tahoma` reviews the fixtures with a system font.
        if let Ok(font) = std::env::var("GALLERY_FONT") {
            unity_hub_ui::fonts::set_family(&font);
        }
        // `GALLERY_TEXT_SCALE=1.5` reviews every fixture at a larger text size.
        if let Some(scale) = std::env::var("GALLERY_TEXT_SCALE")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
        {
            for fixture in &mut fixtures {
                fixture.view_model.text_scale = scale;
            }
        }
        // `GALLERY_FIXTURE=<index>` opens a specific fixture.
        let selected = std::env::var("GALLERY_FIXTURE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| *index < fixtures.len())
            .unwrap_or(0);
        let capture_dir = std::env::var_os("GALLERY_SCREENSHOTS").map(PathBuf::from);

        let task = if capture_dir.is_some() {
            settle()
        } else {
            Task::none()
        };

        (
            Self {
                fixtures,
                selected,
                capture_dir,
            },
            task,
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Select(name) => {
                if let Some(index) = self
                    .fixtures
                    .iter()
                    .position(|fixture| fixture.name == name)
                {
                    self.selected = index;
                }
            }
            Message::Ui(action) => {
                let _ = action;
            }
            Message::Capture => {
                return iced::window::latest()
                    .and_then(iced::window::screenshot)
                    .map(Message::Captured);
            }
            Message::Captured(shot) => {
                let Some(dir) = &self.capture_dir else {
                    return Task::none();
                };
                // Raw RGBA keeps the gallery free of image encoders; convert with
                // `magick -size WxH -depth 8 rgba:<file> out.png`.
                let file = dir.join(format!(
                    "{:02}-{}x{}.rgba",
                    self.selected, shot.size.width, shot.size.height
                ));
                if let Err(error) =
                    std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&file, &shot.rgba))
                {
                    eprintln!("failed to write {}: {error}", file.display());
                }

                if self.selected + 1 < self.fixtures.len() {
                    self.selected += 1;
                    return settle();
                }
                return iced::exit();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let names = self
            .fixtures
            .iter()
            .map(|fixture| fixture.name)
            .collect::<Vec<_>>();
        let current = self.fixtures[self.selected].name;

        let controls = container(
            row![
                iced::widget::text("Design gallery")
                    .size(16)
                    .font(unity_hub_ui::fonts::regular()),
                space().width(Fill),
                pick_list(names, Some(current), Message::Select)
                    .text_size(13.0)
                    .font(unity_hub_ui::fonts::regular())
                    .width(280),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        )
        .padding(10)
        .width(Fill)
        .style(unity_hub_ui::theme::card);

        column![
            controls,
            unity_hub_ui::view(&self.fixtures[self.selected].view_model).map(Message::Ui),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn theme(&self) -> Theme {
        unity_hub_ui::theme()
    }
}
