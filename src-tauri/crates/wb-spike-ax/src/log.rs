// SPDX-License-Identifier: GPL-3.0-or-later
//! `events.jsonl`: one JSON object per line, flushed as it is written so a
//! run stopped with Ctrl+C keeps everything up to that point. Every line has
//! `t_ms` (milliseconds since the session started, monotonic) and `kind`.
//! Copied from wb-spike-fn (RS-01).

use serde_json::{Map, Value};
use std::fs::File;
use std::io::{LineWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

pub struct Log {
    epoch: Instant,
    out: Mutex<Option<LineWriter<File>>>,
}

impl Log {
    pub fn create(path: &Path, epoch: Instant) -> std::io::Result<Log> {
        Ok(Log {
            epoch,
            out: Mutex::new(Some(LineWriter::new(File::create(path)?))),
        })
    }

    /// Milliseconds (with µs precision) since the session started.
    pub fn ms(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.epoch).as_micros() as f64 / 1000.0
    }

    pub fn write(&self, kind: &str, at: Instant, fields: Value) {
        let line = render(kind, self.ms(at), fields);
        if let Ok(mut guard) = self.out.lock() {
            if let Some(w) = guard.as_mut() {
                let _ = writeln!(w, "{line}");
            }
        }
    }
}

fn render(kind: &str, t_ms: f64, fields: Value) -> String {
    let mut obj = Map::new();
    obj.insert("t_ms".into(), Value::from((t_ms * 1000.0).round() / 1000.0));
    obj.insert("kind".into(), Value::from(kind));
    match fields {
        Value::Object(m) => obj.extend(m),
        Value::Null => {}
        other => {
            obj.insert("value".into(), other);
        }
    }
    Value::Object(obj).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lines_carry_time_and_kind_first() {
        let line = render("mic_open", 151.2345, json!({"take": 3}));
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["kind"], "mic_open");
        assert_eq!(v["t_ms"], 151.235);
        assert_eq!(v["take"], 3);
    }

    #[test]
    fn writes_to_file() {
        let dir = std::env::temp_dir().join(format!("wb-spike-ax-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let log = Log::create(&path, Instant::now()).unwrap();
        log.write("a", Instant::now(), json!({"x": 1}));
        log.write("b", Instant::now(), Value::Null);
        drop(log);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
