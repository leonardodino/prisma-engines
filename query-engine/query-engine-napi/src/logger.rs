mod channel;
mod registry;
mod telemetry;
mod visitor;

use channel::EventChannel;
use napi::threadsafe_function::ThreadsafeFunction;
use opentelemetry::global;
use opentelemetry_jaeger::{Propagator, Uninstall};
use registry::EventRegistry;
use std::{future::Future, sync::Arc};
use telemetry::WithTelemetry;
use tracing::level_filters::LevelFilter;
use tracing_futures::WithSubscriber;
use tracing_subscriber::layer::{Layered, SubscriberExt};

#[derive(Clone)]
enum Subscriber {
    Normal(Layered<EventChannel, EventRegistry>),
    WithTelemetry(WithTelemetry),
}

/// A logger logging to a bounded channel. When in scope, all log messages from
/// the scope are stored to the channel, which must be consumed or after some
/// point, further log lines will just be dropped.
#[derive(Clone)]
pub struct ChannelLogger {
    subscriber: Subscriber,
    level: LevelFilter,
    guard: Option<Arc<Uninstall>>,
}

impl ChannelLogger {
    /// Creates a new instance of a logger with the minimum log level.
    pub fn new(level: LevelFilter, callback: ThreadsafeFunction<String>) -> Self {
        let mut javascript_cb = EventChannel::new(callback);
        javascript_cb.filter_level(level);

        let subscriber = Subscriber::Normal(EventRegistry::new().with(javascript_cb));

        Self {
            subscriber,
            level,
            guard: None,
        }
    }

    /// Creates a new instance of a logger with the `trace` minimum level.
    /// Enables tracing events to Jaeger.
    pub fn new_with_telemetry(callback: ThreadsafeFunction<String>) -> Self {
        let mut javascript_cb = EventChannel::new(callback);
        javascript_cb.filter_level(LevelFilter::TRACE);

        global::set_text_map_propagator(Propagator::new());

        let (tracer, guard) = opentelemetry_jaeger::new_pipeline()
            .with_service_name("Query Engine")
            .install()
            .expect("meow");

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        let registry = EventRegistry::new().with(telemetry_layer).with(javascript_cb);
        let with_telemetry = WithTelemetry::new(registry);

        let subscriber = Subscriber::WithTelemetry(with_telemetry);

        Self {
            subscriber,
            level: LevelFilter::TRACE,
            guard: Some(Arc::new(guard)),
        }
    }

    /// Wraps a future to a logger, storing all events in the pipeline to
    /// the channel.
    pub async fn with_logging<F, U, T>(&self, f: F) -> crate::Result<T>
    where
        U: Future<Output = crate::Result<T>>,
        F: FnOnce() -> U,
    {
        match self.subscriber {
            Subscriber::Normal(ref subscriber) => f().with_subscriber(subscriber.clone()).await,
            Subscriber::WithTelemetry(ref subscriber) => f().with_subscriber(subscriber.clone()).await,
        }
    }
}
