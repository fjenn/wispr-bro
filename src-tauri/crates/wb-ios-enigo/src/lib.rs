// SPDX-License-Identifier: GPL-3.0-or-later
//
//! See Cargo.toml. Every call fails with [`InputError::Unavailable`];
//! `Enigo::new` fails too, so the shared code never holds one.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputError {
    Unavailable,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("input simulation is not available on iOS")
    }
}

impl std::error::Error for InputError {}

pub type InputResult<T> = Result<T, InputError>;
pub type NewConError = InputError;

#[derive(Debug, Clone, Default)]
pub struct Settings {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Press,
    Release,
    Click,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Alt,
    Backspace,
    Control,
    Delete,
    DownArrow,
    End,
    Escape,
    Home,
    LeftArrow,
    Meta,
    Option,
    Return,
    RightArrow,
    Shift,
    Space,
    Tab,
    UpArrow,
    Unicode(char),
    Other(u32),
}

#[derive(Debug)]
pub struct Enigo {
    _private: (),
}

impl Enigo {
    pub fn new(_settings: &Settings) -> Result<Self, NewConError> {
        Err(InputError::Unavailable)
    }
}

pub trait Keyboard {
    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()>;
    fn text(&mut self, text: &str) -> InputResult<()>;
    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()>;
}

pub trait Mouse {
    fn location(&self) -> InputResult<(i32, i32)>;
}

impl Keyboard for Enigo {
    fn key(&mut self, _key: Key, _direction: Direction) -> InputResult<()> {
        Err(InputError::Unavailable)
    }
    fn text(&mut self, _text: &str) -> InputResult<()> {
        Err(InputError::Unavailable)
    }
    fn raw(&mut self, _keycode: u16, _direction: Direction) -> InputResult<()> {
        Err(InputError::Unavailable)
    }
}

impl Mouse for Enigo {
    fn location(&self) -> InputResult<(i32, i32)> {
        Err(InputError::Unavailable)
    }
}
