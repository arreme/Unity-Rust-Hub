mod app;
mod jobs;
mod settings;
mod theme;
mod ui;
mod util;
mod views;

use std::future::Future;

/// iced's default executor creates one worker per CPU. All real work in this
/// app runs on the dedicated CLI reader or job threads, so two executor
/// threads are enough and keep the thread/stack footprint small.
struct SmallExecutor(futures::executor::ThreadPool);

impl iced::Executor for SmallExecutor {
    fn new() -> Result<Self, std::io::Error> {
        futures::executor::ThreadPoolBuilder::new()
            .pool_size(2)
            .create()
            .map(Self)
    }

    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        self.0.spawn_ok(future);
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}

fn main() -> iced::Result {
    let icon =
        iced::window::icon::from_rgba(include_bytes!("../assets/icon-128.rgba").to_vec(), 128, 128)
            .ok();

    iced::application(app::App::boot, app::App::update, app::App::view)
        .executor::<SmallExecutor>()
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
