#[cfg(target_os = "android")]
mod android_tracing;

pub mod prelude {
    pub use bevy_utils::tracing::{
        debug, debug_span, error, error_span, info, info_span, trace, trace_span, warn, warn_span,
    };
}
pub use bevy_utils::tracing::{
    debug, debug_span, error, error_span, info, info_span, trace, trace_span, warn, warn_span,
    Level,
};

use bevy_app::{AppBuilder, Plugin};
#[cfg(feature = "tracing-chrome")]
use tracing_subscriber::fmt::{format::DefaultFields, FormattedFields};

use tracing_subscriber::{prelude::*, registry::Registry, EnvFilter};
use tracing_subscriber::{fmt, fmt::format};
use std::sync::{Arc, RwLock};

/// Adds logging to Apps.
#[derive(Default)]
pub struct LogPlugin;

/// LogPlugin settings
pub struct LogSettings {
    /// Filters logs using the [EnvFilter] format
    pub filter: String,

    /// Filters out logs that are "less than" the given level.
    /// This can be further filtered using the `filter` setting.
    pub level: Level,

    /// String that is prepended to the main log message, which you can change with `LogSettings::set_dynamic_prefix`
    pub dynamic_prefix: Arc<RwLock<String>>,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            filter: "wgpu=error".to_string(),
            level: Level::INFO,
            dynamic_prefix: Arc::new(RwLock::new(String::new())),
        }
    }
}

impl LogSettings {
    pub fn set_dynamic_prefix(&self, mut msg: String) {
        if msg.len() > 0 {
            msg.push(' '); // spacing for when we render it before the message field
        }
        *self.dynamic_prefix.write().expect("dynamic_prefix log poisoned") = msg;
    }
}

impl Plugin for LogPlugin {
    fn build(&self, app: &mut AppBuilder) {
        let (default_filter, dynamic_prefix) = {
            let settings = app
                .world_mut()
                .get_resource_or_insert_with(LogSettings::default);
            (format!("{},{}", settings.level, settings.filter), settings.dynamic_prefix.clone())
        };

        let filter_layer = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new(&default_filter))
            .unwrap();

        let subscriber = Registry::default().with(filter_layer);

        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
        {
            // moving dynamic_prefix clone into closure
            let dynamic_prefix = dynamic_prefix.clone();
            let formatter = format::debug_fn(move |writer, field, value| {
                // when rendering the message field (the main bit of text in the info!(...) call)
                // prepend the dynamic_prefix
                if field.name() == "message" {
                    write!(writer, "{}{:?}", *dynamic_prefix.read().expect("extra lock poisoned"), value)
                } else {
                    // additional fields just printed as key=val
                    write!(writer, "{}={:?}", field, value)
                }
            })
            .delimited(", ");

            // use default formatter, but replace field format with our custom one that prefixes message
            let fmt_layer = fmt::Layer::default().fmt_fields(formatter);
            let subscriber = subscriber.with(fmt_layer);

            #[cfg(feature = "tracing-chrome")]
            {
                let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
                    .name_fn(Box::new(|event_or_span| match event_or_span {
                        tracing_chrome::EventOrSpan::Event(event) => event.metadata().name().into(),
                        tracing_chrome::EventOrSpan::Span(span) => {
                            if let Some(fields) =
                                span.extensions().get::<FormattedFields<DefaultFields>>()
                            {
                                format!("{}: {}", span.metadata().name(), fields.fields.as_str())
                            } else {
                                span.metadata().name().into()
                            }
                        }
                    }))
                    .build();
                app.world_mut().insert_non_send(guard);
                let subscriber = subscriber.with(chrome_layer);
                bevy_utils::tracing::subscriber::set_global_default(subscriber)
                    .expect("Could not set global default tracing subscriber. If you've already set up a tracing subscriber, please disable LogPlugin from Bevy's DefaultPlugins");
            }

            #[cfg(not(feature = "tracing-chrome"))]
            {
                bevy_utils::tracing::subscriber::set_global_default(subscriber)
                    .expect("Could not set global default tracing subscriber. If you've already set up a tracing subscriber, please disable LogPlugin from Bevy's DefaultPlugins");
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            console_error_panic_hook::set_once();
            let subscriber = subscriber.with(tracing_wasm::WASMLayer::new(
                tracing_wasm::WASMLayerConfig::default(),
            ));
            bevy_utils::tracing::subscriber::set_global_default(subscriber)
                .expect("Could not set global default tracing subscriber. If you've already set up a tracing subscriber, please disable LogPlugin from Bevy's DefaultPlugins");
        }

        #[cfg(target_os = "android")]
        {
            let subscriber = subscriber.with(android_tracing::AndroidLayer::default());
            bevy_utils::tracing::subscriber::set_global_default(subscriber)
                .expect("Could not set global default tracing subscriber. If you've already set up a tracing subscriber, please disable LogPlugin from Bevy's DefaultPlugins");
        }
    }
}
