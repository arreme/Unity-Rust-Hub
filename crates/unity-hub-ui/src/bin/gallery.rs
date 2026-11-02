use iced::widget::{column, container, pick_list, row, space};
use iced::{Element, Fill, Task, Theme};

use unity_hub_model::UiAction;
use unity_hub_ui::fixtures::{self, Fixture};

fn main() -> iced::Result {
    iced::application(Gallery::boot, Gallery::update, Gallery::view)
        .title("Unity Rust Hub · design gallery")
        .theme(Gallery::theme)
        .window(iced::window::Settings {
            size: unity_hub_ui::WINDOW_SIZE,
            min_size: Some(unity_hub_ui::MIN_WINDOW_SIZE),
            ..Default::default()
        })
        .centered()
        .run()
}

struct Gallery {
    fixtures: Vec<Fixture>,
    selected: usize,
}

#[derive(Debug, Clone)]
enum Message {
    Select(&'static str),
    Ui(UiAction),
}

impl Gallery {
    fn boot() -> (Self, Task<Message>) {
        (
            Self {
                fixtures: fixtures::all(),
                selected: 0,
            },
            Task::none(),
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
                iced::widget::text("Design gallery").size(16),
                space().width(Fill),
                pick_list(names, Some(current), Message::Select)
                    .text_size(13.0)
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
