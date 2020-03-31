use chrono::SecondsFormat;
use chrono::Utc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Error;
use std::fmt::Formatter;
use std::fmt::Write;
use std::io;
use std::sync::Mutex;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span::Attributes;
use tracing::span::Record;
use tracing::subscriber::set_global_default;
use tracing::subscriber::SetGlobalDefaultError;
use tracing::Event;
use tracing::Id;
use tracing::Subscriber;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::Layer;

pub use log::*;
pub use tracing::field::display;
pub use tracing::field::Empty;
pub use tracing::span;
pub use tracing::Level;

struct EventWriter<'a>(&'a mut String);

impl Visit for EventWriter<'_> {
    fn record_debug(&mut self, f: &Field, value: &dyn Debug) {
        match f.name() {
            "message" => {
                let _ = write!(self.0, "{:?}", value);
            }
            _ => return,
        }
    }
}

pub struct FallLog<W: io::Write> {
    writer: Mutex<W>,
    max_level: Level,
    app_name: String,
    extend_fields: Vec<String>,
}

impl<W> FallLog<W>
where
    W: io::Write + Send + 'static,
{
    pub fn new(app_name: String, make_writer: W) -> Self {
        FallLog {
            writer: Mutex::new(make_writer),
            max_level: Level::INFO,
            app_name,
            extend_fields: vec![],
        }
    }

    pub fn max_level(self, level: Level) -> Self {
        FallLog {
            writer: self.writer,
            max_level: level,
            app_name: self.app_name,
            extend_fields: self.extend_fields,
        }
    }

    pub fn add_field(self, field_name: String) -> Self {
        let mut f = self.extend_fields;
        f.push(field_name);
        FallLog {
            writer: self.writer,
            max_level: self.max_level,
            app_name: self.app_name,
            extend_fields: f,
        }
    }

    pub fn init(self) -> Result<(), SetGlobalDefaultError> {
        let subscriber = Registry::default().with(self);
        let _ = tracing_log::LogTracer::init();
        set_global_default(subscriber)
    }
}

pub struct ExtendedLog {
    data: HashMap<String, String>,
    keys: Vec<String>,
}

impl Default for ExtendedLog {
    fn default() -> Self {
        ExtendedLog {
            data: HashMap::new(),
            keys: vec![
                "trace_id".to_string(),
                "span_id".to_string(),
                "parent_span_id".to_string(),
            ],
        }
    }
}

impl Visit for ExtendedLog {
    fn record_debug(&mut self, f: &Field, d: &dyn Debug) {
        let name = f.name().to_owned();
        if self.keys.contains(&name) {
            self.data.insert(name, format!("{:?}", d));
        }
    }
}

impl Display for ExtendedLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let mut flag = false;
        for k in self.keys.iter() {
            if flag {
                f.write_char(',')?;
            } else {
                flag = true;
            }
            if let Some(v) = self.data.get(k) {
                write!(f, "{}", v)?;
            }
        }
        Ok(())
    }
}

impl<S: Subscriber, W> Layer<S> for FallLog<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: io::Write + 'static,
{
    fn enabled(&self, metadata: &tracing::metadata::Metadata<'_>, _: Context<'_, S>) -> bool {
        metadata.level() <= &self.max_level
    }

    fn new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        let mut info = ExtendedLog::default();
        for k in self.extend_fields.iter() {
            info.keys.push(k.to_owned());
        }
        attrs.record(&mut info);
        extensions.insert(info);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        if let Some(sl) = extensions.get_mut::<ExtendedLog>() {
            values.record(sl);
        }
    }
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        thread_local! {
            static BUF: RefCell<String> = RefCell::new(String::new());
        }

        BUF.with(|buf| {
            let borrow = buf.try_borrow_mut();
            let mut a;
            let mut b;
            let mut buf = match borrow {
                Ok(buf) => {
                    a = buf;
                    &mut *a
                }
                _ => {
                    b = String::new();
                    &mut b
                }
            };
            let _ = write!(
                &mut buf,
                "{} {}",
                Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                event.metadata().level()
            );
            let mut flag = false;
            if let Some(id) = ctx.current_span().id() {
                let span = ctx.span(id).expect("Span not found, this is a bug");
                let extensions = span.extensions();
                if let Some(info) = extensions.get::<ExtendedLog>() {
                    let _ = write!(&mut buf, " [{},{}]", self.app_name, info);
                    flag = true;
                }
            }
            if !flag {
                let _ = write!(&mut buf, " [{},]", self.app_name);
            }
            let _ = write!(
                &mut buf,
                " {}: ",
                event.metadata().module_path().unwrap_or("")
            );
            event.record(&mut EventWriter(&mut buf));
            let _ = writeln!(&mut buf);
            let _ = self.writer.lock().unwrap().write_all(buf.as_bytes());
            buf.clear();
        });
    }
}
