#![allow(dead_code)]

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub struct BufWriter(Arc<Mutex<Vec<u8>>>);

impl Write for BufWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.0.lock().unwrap();
        guard.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct Buffer(Arc<Mutex<Vec<u8>>>);

impl<'writer> MakeWriter<'writer> for Buffer {
    type Writer = BufWriter;
    fn make_writer(&'writer self) -> Self::Writer {
        BufWriter(self.0.clone())
    }
}

#[derive(Clone, Debug)]
pub struct CapturedLogs {
    buffer: Arc<Mutex<Vec<u8>>>,
}

/// Run the given closure under a scoped fmt subscriber that writes into a buffer.
/// Returns the closure's result along with captured logs as a single string.
pub fn with_captured_logs<R>(f: impl FnOnce() -> R) -> (R, CapturedLogs) {
    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = Buffer(buffer.clone());
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::new("debug"))
        .with_target(false)
        .with_writer(writer)
        .finish();

    // Set global subscriber; ignore error if already set by other tests
    let _ = subscriber.try_init();
    let result = f();
    (result, CapturedLogs { buffer })
}

impl CapturedLogs {
    pub fn snapshot(&self) -> String {
        String::from_utf8_lossy(&self.buffer.lock().unwrap()).to_string()
    }
}

pub fn count_level(logs: &CapturedLogs, level: Level) -> usize {
    let key = match level {
        Level::ERROR => "ERROR",
        Level::WARN => "WARN",
        Level::INFO => "INFO",
        Level::DEBUG => "DEBUG",
        Level::TRACE => "TRACE",
    };
    logs.snapshot().matches(key).count()
}

pub fn contains_message(logs: &CapturedLogs, needle: &str) -> bool {
    logs.snapshot().contains(needle)
}
