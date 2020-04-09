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
use tracing::Metadata;
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
pub use tracing_subscriber::registry::SpanRef;

const TRACE_ID: &str = "trace_id";
const SPAN_ID: &str = "span_id";
const PARENT_SPAN_ID: &str = "parent_span_id";
pub const PADDING: &str = "padding";

/// Open tracing struct.
///
///
pub struct OpenTrace {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: String,
}

fn rand_u64() -> u64 {
    rand::random::<u64>()
}

fn u64_hex(i: u64) -> String {
    format!("{:016x}", i)
}

#[cfg(test)]
extern crate quickcheck;
#[cfg(test)]
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_rand() {
        assert_ne!(rand_u64(), rand_u64());
    }

    #[quickcheck]
    fn test_hex_len(i: u64) {
        assert_eq!(16, u64_hex(i).len());
    }
}

impl Default for OpenTrace {
    fn default() -> Self {
        let trace_id = u64_hex(rand_u64());
        let span_id = trace_id.clone();
        OpenTrace {
            trace_id,
            span_id,
            parent_span_id: String::from(""),
        }
    }
}

impl OpenTrace {
    pub fn new(trace_id: u64, span_id: u64, parent_span_id: Option<u64>) -> Self {
        OpenTrace {
            trace_id: u64_hex(trace_id),
            span_id: u64_hex(span_id),
            parent_span_id: parent_span_id.map(u64_hex).unwrap_or_else(|| "".into()),
        }
    }

    pub fn from_parent(trace_id: u64, parent_span_id: Option<u64>) -> Self {
        OpenTrace::new(trace_id, rand_u64(), parent_span_id)
    }
}

impl From<OpenTrace> for span::Span {
    fn from(ot: OpenTrace) -> Self {
        span!(
            Level::INFO,
            "new_span",
            trace_id = %ot.trace_id,
            span_id = %ot.span_id,
            parent_span_id = %ot.parent_span_id,
            padding = Empty,
        )
    }
}

struct EventWriter<'a>(&'a mut String);

impl Visit for EventWriter<'_> {
    fn record_debug(&mut self, f: &Field, value: &dyn Debug) {
        if let "message" = f.name() {
            let _ = write!(self.0, "{:?}", value);
        }
    }
}

pub fn current_trace_id() -> Option<String> {
    let id = &span::Span::current().id()?;
    tracing::dispatcher::get_default(|r| {
        let span = r.downcast_ref::<Registry>()?.span(id)?;
        let ext = span.extensions();
        ext.get::<ExtendedLog>()?
            .data
            .get(TRACE_ID)
            .map(Clone::clone)
    })
}

pub fn new_child_span() -> Option<OpenTrace> {
    let id = &span::Span::current().id()?;
    tracing::dispatcher::get_default(|r| {
        let span = r.downcast_ref::<Registry>()?.span(id)?;
        let ext = span.extensions();
        let map = &ext.get::<ExtendedLog>()?.data;
        Some(OpenTrace {
            trace_id: map.get(TRACE_ID).map(Clone::clone)?,
            span_id: u64_hex(rand_u64()),
            parent_span_id: map.get(SPAN_ID).map(Clone::clone)?,
        })
    })
}

/// FallLog.
///
/// A layer used to format normal log.
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
            max_level: level,
            ..self
        }
    }

    pub fn add_field(self, field_name: String) -> Self {
        let mut f = self.extend_fields;
        f.push(field_name);
        FallLog {
            extend_fields: f,
            ..self
        }
    }

    pub fn init(self) -> Result<(), SetGlobalDefaultError> {
        let subscriber = Registry::default().with(self);
        let _ = tracing_log::LogTracer::init();
        set_global_default(subscriber)
    }
}

/// Extended log.
///
pub struct ExtendedLog {
    data: HashMap<String, String>,
    keys: Vec<String>,
}

impl Default for ExtendedLog {
    fn default() -> Self {
        ExtendedLog {
            data: HashMap::new(),
            keys: vec![
                TRACE_ID.to_string(),
                SPAN_ID.to_string(),
                PARENT_SPAN_ID.to_string(),
                PADDING.to_string(),
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
    fn enabled(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
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
            let _ = self
                .writer
                .lock()
                .expect("Writer lock failed")
                .write_all(buf.as_bytes());
            buf.clear();
        });
    }
}
