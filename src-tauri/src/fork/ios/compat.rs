// SPDX-License-Identifier: GPL-3.0-or-later
//
//! Desktop-only window calls as no-ops, so shared code that sizes, centres
//! or raises a window compiles for the iPhone unchanged. tauri leaves these
//! methods out of its mobile build; an iPhone app has one full-screen window,
//! so there is nothing for them to do. Call sites import the traits with
//! `#[cfg(target_os = "ios")] use crate::fork::ios::compat::*;`.

use tauri::{Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

pub trait DesktopWindow {
    fn unminimize(&self) -> tauri::Result<()> {
        Ok(())
    }
    fn maximize(&self) -> tauri::Result<()> {
        Ok(())
    }
    fn unmaximize(&self) -> tauri::Result<()> {
        Ok(())
    }
    fn center(&self) -> tauri::Result<()> {
        Ok(())
    }
}

impl<R: Runtime> DesktopWindow for WebviewWindow<R> {}

macro_rules! builder_noops {
    ($($name:ident),* $(,)?) => {
        pub trait DesktopWindowBuilder: Sized {
            $(fn $name(self, _value: bool) -> Self { self })*
        }
    };
}

builder_noops!(
    maximizable,
    minimizable,
    closable,
    always_on_top,
    shadow,
    decorations,
    skip_taskbar,
    transparent,
    accept_first_mouse,
    focused,
    focusable,
    visible_on_all_workspaces,
);

impl<R: Runtime, M: Manager<R>> DesktopWindowBuilder for WebviewWindowBuilder<'_, R, M> {}
