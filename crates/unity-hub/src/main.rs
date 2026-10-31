mod app;
mod jobs;
mod settings;
mod theme;
mod util;
mod views;

fn main() -> iced::Result {
    let icon = iced::window::icon::from_file_data(include_bytes!("../assets/icon.png"), None).ok();

    iced::application(app::App::boot, app::App::update, app::App::view)
        .title("Unity Rust Hub")
        .theme(app::App::theme)
        .subscription(app::App::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(1280.0, 840.0),
            min_size: Some(iced::Size::new(1040.0, 680.0)),
            icon,
            ..Default::default()
        })
        .centered()
        .run()
}
