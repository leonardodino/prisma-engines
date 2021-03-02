use opentelemetry::global;
use opentelemetry_jaeger::{Propagator, Uninstall};
use tide_server_timing::TimingLayer;
use tracing::{dispatcher::SetGlobalDefaultError, subscriber};
use tracing_subscriber::{layer::SubscriberExt, registry::LookupSpan, EnvFilter, FmtSubscriber};

use crate::LogFormat;

type LoggerResult<T> = Result<T, SetGlobalDefaultError>;

/// An installer for a global logger.
#[derive(Debug, Clone, Copy)]
pub struct Logger {
    service_name: &'static str,
    log_format: LogFormat,
    enable_telemetry: bool,
}

impl Logger {
    /// Initialize a new global logger installer.
    pub fn new(service_name: &'static str) -> Self {
        Self {
            service_name,
            log_format: LogFormat::Json,
            enable_telemetry: false,
        }
    }

    /// Sets the STDOUT log output format. Default: Json.
    pub fn log_format(&mut self, log_format: LogFormat) {
        self.log_format = log_format;
    }

    /// Enables Jaeger telemetry.
    pub fn enable_telemetry(&mut self, enable_telemetry: bool) {
        self.enable_telemetry = enable_telemetry;
    }

    /// Install logger as a global. Can be called only once per application
    /// instance. The returned guard value needs to stay in scope for the whole
    /// lifetime of the service.
    pub fn install(self) -> LoggerResult<Option<Uninstall>> {
        // Enable `tide` logs to be captured.
        let filter = EnvFilter::from_default_env().add_directive("tide=info".parse().unwrap());

        match self.log_format {
            LogFormat::Text => {
                let subscriber = FmtSubscriber::builder()
                    .with_max_level(tracing::Level::TRACE)
                    .finish()
                    .with(TimingLayer::new());

                self.finalize(subscriber)
            }
            LogFormat::Json => {
                let builder = FmtSubscriber::builder().json();

                if self.enable_telemetry {
                    let subscriber = builder
                        .with_max_level(tracing::Level::TRACE)
                        .finish()
                        .with(TimingLayer::new());

                    self.finalize(subscriber)
                } else {
                    let subscriber = builder.with_env_filter(filter).finish().with(TimingLayer::new());

                    self.finalize(subscriber)
                }
            }
        }
    }

    fn finalize<T>(self, subscriber: T) -> LoggerResult<Option<Uninstall>>
    where
        T: SubscriberExt + Send + Sync + 'static + for<'span> LookupSpan<'span>,
    {
        if self.enable_telemetry {
            global::set_text_map_propagator(Propagator::new());

            let (tracer, guard) = opentelemetry_jaeger::new_pipeline()
                .with_service_name(self.service_name)
                .install()
                .expect("meow");

            let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
            subscriber::set_global_default(subscriber.with(telemetry))?;

            Ok(Some(guard))
        } else {
            subscriber::set_global_default(subscriber)?;

            Ok(None)
        }
    }
}
