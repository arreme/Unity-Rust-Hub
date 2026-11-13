// A GUI binary must not open a console window. Debug builds keep it so
// `cargo run` can show panics and backtraces.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod runtime;

use std::future::Future;

use runtime::Runtime;

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

/// iced picks the first adapter from `WGPU_BACKEND`, falling back to every
/// primary backend. On Windows that enumeration can land on Vulkan, which costs
/// roughly 60 MB more resident memory than DX12 for the same 2D scene. Prefer
/// the native backend unless the user has explicitly chosen one.
fn prefer_native_gpu_backend() {
    #[cfg(target_os = "windows")]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        std::env::set_var("WGPU_BACKEND", "dx12");
    }
}

fn main() -> iced::Result {
    prefer_native_gpu_backend();

    let icon =
        iced::window::icon::from_rgba(include_bytes!("../assets/icon-128.rgba").to_vec(), 128, 128)
            .ok();

    let mut app = iced::application(Runtime::boot, Runtime::update, Runtime::view);
    for font in unity_hub_ui::fonts::ALL {
        app = app.font(font);
    }

    app.executor::<SmallExecutor>()
        .default_font(unity_hub_ui::fonts::SANS)
        .title("Unity Rust Hub")
        .theme(Runtime::theme)
        .subscription(Runtime::subscription)
        .window(iced::window::Settings {
            size: unity_hub_ui::WINDOW_SIZE,
            min_size: Some(unity_hub_ui::MIN_WINDOW_SIZE),
            icon,
            ..Default::default()
        })
        .centered()
        .run()
}
