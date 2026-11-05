// SPDX-License-Identifier: GPL-3.0-or-later
//! Terminal input for the guided session: a reader thread feeds lines into a
//! channel; waiting for a line keeps the main run loop serviced and wakes on
//! Control+C.

use serde::Serialize;
use std::io::Write;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use crate::macos;

pub enum Msg {
    Line(String),
    Eof,
    Wake,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ans {
    Yes,
    No,
    Unsure,
}

pub struct Ui {
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    eof: std::cell::Cell<bool>,
}

impl Ui {
    pub fn new() -> Ui {
        let (tx, rx) = mpsc::channel();
        let reader = tx.clone();
        let _ = std::thread::Builder::new()
            .name("wb-rs05-stdin".into())
            .spawn(move || {
                let stdin = std::io::stdin();
                let mut line = String::new();
                loop {
                    line.clear();
                    match stdin.read_line(&mut line) {
                        Ok(0) | Err(_) => {
                            let _ = reader.send(Msg::Eof);
                            return;
                        }
                        Ok(_) => {
                            if reader.send(Msg::Line(line.clone())).is_err() {
                                return;
                            }
                        }
                    }
                }
            });
        Ui {
            rx,
            tx,
            eof: std::cell::Cell::new(false),
        }
    }

    /// A handle the signal thread uses to wake a waiting prompt.
    pub fn waker(&self) -> impl Fn() + Send + 'static {
        let tx = self.tx.clone();
        move || {
            let _ = tx.send(Msg::Wake);
        }
    }

    /// The next line, or None on end of input / interrupt.
    pub fn line(&self) -> Option<String> {
        loop {
            if self.eof.get() || crate::owner::interrupted() {
                return None;
            }
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(Msg::Line(l)) => return Some(l.trim().to_string()),
                Ok(Msg::Wake) => continue,
                Ok(Msg::Eof) | Err(RecvTimeoutError::Disconnected) => {
                    self.eof.set(true);
                    return None;
                }
                Err(RecvTimeoutError::Timeout) => macos::pump(Duration::from_millis(1)),
            }
        }
    }

    /// A line typed meanwhile, without waiting.
    pub fn poll_line(&self) -> Option<String> {
        match self.rx.try_recv() {
            Ok(Msg::Line(l)) => Some(l.trim().to_string()),
            Ok(Msg::Eof) => {
                self.eof.set(true);
                None
            }
            _ => None,
        }
    }

    /// Throws away typed-ahead input.
    pub fn drain(&self) {
        // Typed-ahead (or pasted) terminal input that has no Return yet.
        // SAFETY: tcflush on fd 0; harmless if stdin is not a terminal.
        unsafe {
            libc::tcflush(0, libc::TCIFLUSH);
        }
        while let Ok(m) = self.rx.try_recv() {
            if matches!(m, Msg::Eof) {
                self.eof.set(true);
            }
        }
    }

    pub fn enter(&self, prompt: &str) -> Option<String> {
        self.drain();
        print!("{prompt} ");
        let _ = std::io::stdout().flush();
        self.line()
    }

    pub fn yes_no(&self, q: &str) -> Option<Ans> {
        self.drain();
        loop {
            print!("  ? {q}\n    (y = yes, n = no, u = not sure) ");
            let _ = std::io::stdout().flush();
            let l = self.line()?;
            match l.to_lowercase().as_str() {
                "y" | "yes" => return Some(Ans::Yes),
                "n" | "no" => return Some(Ans::No),
                "u" | "?" | "not sure" => return Some(Ans::Unsure),
                _ => println!("    Please type y, n or u and press Enter."),
            }
        }
    }

    pub fn choice(&self, q: &str, options: &[&str]) -> Option<usize> {
        self.drain();
        loop {
            println!("  ? {q}");
            for (i, o) in options.iter().enumerate() {
                println!("      {}  {o}", i + 1);
            }
            print!("    Type a number and press Enter: ");
            let _ = std::io::stdout().flush();
            let l = self.line()?;
            if let Ok(n) = l.parse::<usize>() {
                if (1..=options.len()).contains(&n) {
                    return Some(n - 1);
                }
            }
        }
    }
}
