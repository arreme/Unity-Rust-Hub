use iced::{Element, Subscription, Task, Theme};

use unity_hub_controller::{App, Message};
use unity_hub_model::ViewModel;

/// Composition root: owns the controller, caches the last view model, and
/// connects the visual layer's actions to the controller's message loop.
pub struct Runtime {
    app: App,
    view_model: ViewModel,
}

impl Runtime {
    pub fn boot() -> (Self, Task<Message>) {
        let (app, task) = App::boot();
        let view_model = app.view_model();
        (Self { app, view_model }, task)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.app.update(message);
        self.view_model = self.app.view_model();
        task
    }

    pub fn view(&self) -> Element<'_, Message> {
        unity_hub_ui::view(&self.view_model).map(Message::from)
    }

    pub fn theme(&self) -> Theme {
        unity_hub_ui::theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        self.app.subscription()
    }
}
