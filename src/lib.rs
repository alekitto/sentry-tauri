#[cfg(feature = "panic")]
mod panic;

use sentry::{add_breadcrumb, capture_event, protocol::Event, Breadcrumb, ClientInitGuard};
use std::time::Duration;
use tauri::{
    generate_handler,
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Runtime,
};

pub use sentry;
pub use sentry::ClientOptions;

#[cfg(feature = "panic")]
pub use panic::PanicIntegration;

#[derive(Debug, Clone)]
pub struct JavaScriptOptions {
    pub inject: bool,
    pub debug: bool,
}

impl Default for JavaScriptOptions {
    fn default() -> Self {
        #[cfg(not(debug_assertions))]
        let debug = false;
        #[cfg(debug_assertions)]
        let debug = true;

        Self {
            inject: true,
            debug,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub javascript: JavaScriptOptions,
    pub client: ClientOptions,
}

#[tauri::command]
fn event<R: Runtime>(_app: AppHandle<R>, mut event: Event<'static>) {
    event.platform = "javascript".into();
    capture_event(event);
}

#[tauri::command]
fn breadcrumb<R: Runtime>(_app: AppHandle<R>, breadcrumb: Breadcrumb) {
    add_breadcrumb(breadcrumb);
}

pub fn init<R>(options: Options) -> TauriPlugin<R>
where
    R: Runtime,
{
    let sentry_client = {
        #[allow(unused_mut)]
        let mut options = options.client;
        if options.default_integrations {
            #[cfg(feature = "panic")]
            options
                .integrations
                .insert(0, std::sync::Arc::new(PanicIntegration::default()))
        }

        sentry::init(options)
    };

    let mut plugin_builder = Builder::new("sentry")
        .invoke_handler(generate_handler![event, breadcrumb])
        .setup(|app, _api| {
            app.manage(sentry_client);
            Ok(())
        })
        .on_event(|app, event| {
            if let RunEvent::Exit = event {
                let client = app.state::<ClientInitGuard>();
                client.flush(Some(Duration::from_secs(5)));
            }
        });

    if options.javascript.inject {
        plugin_builder = plugin_builder.js_init_script(
            include_str!("../dist/inject.min.js")
                .replace("__DEBUG__", &format!("{}", options.javascript.debug)),
        );
    }

    plugin_builder.build()
}
