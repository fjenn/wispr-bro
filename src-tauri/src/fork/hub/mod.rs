// SPDX-License-Identifier: GPL-3.0-or-later
//! Hub window chrome (S-13, C-25).
//!
//! Handy builds the `main` window in its `.setup()` closure at 720×600 plus
//! the title bar (`lib.rs`, `utils::MAIN_WINDOW_SIZE`). The Hub needs a
//! larger default and an overlay title bar, and R5 S-13 sets both here, from
//! [`super::on_ready`], so `lib.rs` is not edited:
//!
//! - **Size:** [`HUB_PAGE_SIZE`] (page area, below the title bar), shrunk to
//!   fit the screen's work area; the minimum stays Handy's 720×600 page so
//!   the onboarding screens, which share the window, still fit.
//! - **Title text hidden** (macOS): the Hub draws its own header under the
//!   title bar; the window keeps its title for the Window menu, Mission
//!   Control and VoiceOver.
//! - **Overlay title bar** (macOS) on every screen, Hub and onboarding
//!   (Design Phase 0): the page draws a 46 pt title strip
//!   (`--wb-titlebar-height`) and the traffic lights are centred in it
//!   ([`TRAFFIC_LIGHTS`], set on the window builder by [`window_builder`],
//!   which tao re-applies on every redraw). The frontend also asks for the
//!   overlay when it mounts. The drag strip needs
//!   `core:window:allow-start-dragging` and the switch
//!   `core:window:allow-set-title-bar-style` (`capabilities/fork.json`).

use tauri::{AppHandle, LogicalSize, Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

/// Where the traffic lights go, in logical points from the window's top-left
/// (tao's inset: `x` is the close button's left edge; `y` sets the title-bar
/// container's height, `y` + the button's height, inside which AppKit places
/// the buttons). REFERENCE-SPEC §1 measures the close button's centre at
/// 21 / 23 pt, the middle of the 46 pt strip. Mac wave 6 measured our close
/// button on macOS 26 (2× window capture): `y` = 23 put its centre at
/// 20.75 pt, 2.25 pt high, so `y` is the strip's middle plus that offset.
pub const TRAFFIC_LIGHTS: (f64, f64) = (
    14.0,
    TITLE_STRIP_HEIGHT / 2.0 + TRAFFIC_LIGHTS_CENTRE_OFFSET,
);

/// How far above tao's `y` the buttons' centre lands on macOS 26 (measured
/// in Mac wave 6, see above; the Scratchpad note window uses it too).
pub const TRAFFIC_LIGHTS_CENTRE_OFFSET: f64 = 2.25;

/// Height of the Hub's title strip; keep in step with
/// `--wb-titlebar-height` in `src/fork/styles/tokens.css`.
pub const TITLE_STRIP_HEIGHT: f64 = 46.0;

/// The main window's builder with the Hub's title bar (macOS: overlay,
/// hidden title text, traffic lights at [`TRAFFIC_LIGHTS`]):
/// called on Handy's builder in `lib.rs` (one hook line) and on the rebuilt
/// window in [`park`]. A no-op elsewhere.
pub fn window_builder<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "macos")]
    {
        builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(
                TRAFFIC_LIGHTS.0,
                TRAFFIC_LIGHTS.1,
            ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        builder
    }
}

/// Default size of the Hub's page area in logical pixels (width × height,
/// title bar excluded).
pub const HUB_PAGE_SIZE: (f64, f64) = (1040.0, 720.0);

/// Smallest page area: Handy's own main-window size, which the onboarding
/// screens are laid out for.
pub const HUB_MIN_PAGE_SIZE: (f64, f64) = crate::utils::MAIN_WINDOW_SIZE;

/// Room kept free around the window when the screen is small.
pub const SCREEN_MARGIN: f64 = 48.0;

/// Window frame sizes (title bar included) for the Hub.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HubWindowSize {
    pub size: (f64, f64),
    pub min: (f64, f64),
}

/// The Hub's default and minimum frame size for a title bar of `bar` logical
/// pixels and a screen work area of `work_area` (logical), if known.
///
/// The default shrinks to fit the work area less [`SCREEN_MARGIN`], but never
/// below the minimum.
pub fn hub_window_size(bar: f64, work_area: Option<(f64, f64)>) -> HubWindowSize {
    let bar = if bar.is_finite() { bar.max(0.0) } else { 0.0 };
    let min = (HUB_MIN_PAGE_SIZE.0, HUB_MIN_PAGE_SIZE.1 + bar);
    let mut size = (HUB_PAGE_SIZE.0, HUB_PAGE_SIZE.1 + bar);
    if let Some((w, h)) = work_area {
        size.0 = size.0.min(w - SCREEN_MARGIN);
        size.1 = size.1.min(h - SCREEN_MARGIN);
    }
    HubWindowSize {
        size: (size.0.max(min.0), size.1.max(min.1)),
        min,
    }
}

/// Title bar height of `window` in logical pixels. Same measure as Handy's
/// `utils::measure_title_bar` (frame minus `contentLayoutRect`, which still
/// excludes the bar under an overlay title bar), for any runtime. Tauri's
/// `outer_size() - inner_size()` reads 0 here on macOS 26, so it is not used.
/// Runs on the main thread (`on_ready` is called from the event loop).
/// Typed to `Wry`: the mock runtime's window handle is not a real NSView, so
/// `on_ready` reaches this only through `ForkRuntime::hub_title_bar_height`.
#[cfg(target_os = "macos")]
pub(crate) fn title_bar_height(window: &WebviewWindow<tauri::Wry>) -> f64 {
    let Ok(ptr) = window.ns_window() else {
        return 0.0;
    };
    // SAFETY: `ptr` is the live NSWindow behind `window`, read on the main
    // thread.
    let ns_window = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
    (ns_window.frame().size.height - ns_window.contentLayoutRect().size.height).max(0.0)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn title_bar_height(_window: &WebviewWindow<tauri::Wry>) -> f64 {
    // Elsewhere the inner size excludes the decorations already.
    0.0
}

/// Logical work area of the monitor the window is on.
fn work_area<R: Runtime>(window: &WebviewWindow<R>) -> Option<(f64, f64)> {
    let monitor = window.current_monitor().ok().flatten()?;
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    Some((
        f64::from(area.size.width) / scale,
        f64::from(area.size.height) / scale,
    ))
}

pub mod park;

/// Applies the Hub's window chrome to `main` and starts parking it while it
/// is hidden (S-38, [`park`]). Called once from `on_ready`.
pub fn on_ready<R: super::ForkRuntime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!("fork hub: no main window at Ready");
        return;
    };
    style_main(app, &window);
    park::watch(app, &window);
}

/// The Hub's size, minimum size and title bar on `window` (Handy's `main`,
/// or the one [`park::ensure_main`] rebuilds).
pub(crate) fn style_main<R: super::ForkRuntime>(_app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let sizes = hub_window_size(R::hub_title_bar_height(window), work_area(window));
    let min = LogicalSize::new(sizes.min.0, sizes.min.1);
    let size = LogicalSize::new(sizes.size.0, sizes.size.1);
    if let Err(e) = window
        .set_min_size(Some(min))
        .and_then(|()| window.set_size(size))
        .and_then(|()| window.center())
    {
        log::warn!("fork hub: could not size the main window: {e}");
    }

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = window.set_title_bar_style(tauri::TitleBarStyle::Overlay) {
            log::warn!("fork hub: overlay title bar failed: {e}");
        }
        R::hub_hide_title_text(window);
    }
    log::info!(
        "fork hub: main window {}×{} (min {}×{})",
        sizes.size.0,
        sizes.size.1,
        sizes.min.0,
        sizes.min.1
    );
}

/// Hides the title text drawn in the title bar (the title itself stays).
#[cfg(target_os = "macos")]
pub(crate) fn hide_title_text(window: &WebviewWindow<tauri::Wry>) {
    let Ok(ptr) = window.ns_window() else {
        return;
    };
    let ptr = ptr as usize;
    let result = window.run_on_main_thread(move || {
        // SAFETY: `ptr` is the live NSWindow behind the main window (Tauri
        // keeps it until the window is destroyed, which S-38's parking does
        // on this same main thread, never in between) and this closure runs
        // on the main thread, as AppKit requires.
        let ns_window = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
        ns_window.setTitleVisibility(objc2_app_kit::NSWindowTitleVisibility::Hidden);
        // Mac wave 2 finding 1: AppKit never saves or restores the Hub.
        crate::fork::restore::not_restorable(ns_window);
    });
    test_keep_painting(window);
    if let Err(e) = result {
        log::warn!("fork hub: could not hide the title text: {e}");
    }
}

/// Mac wave 6: `WISPRBRO_TEST_KEEP_PAINTING=1` keeps the Hub's web view
/// painting while macOS reports its window occluded (a locked screen, a
/// window behind others), so a scripted pass can capture the real WebKit
/// screens with `screencapture -l`. Debug builds of test identities
/// (`com.wisprbro.<slice>check`) only; WebKit's private
/// `_setWindowOcclusionDetectionEnabled:`, used only when it answers it.
#[cfg(target_os = "macos")]
pub(crate) fn test_keep_painting(window: &WebviewWindow<tauri::Wry>) {
    if !cfg!(debug_assertions)
        || std::env::var_os("WISPRBRO_TEST_KEEP_PAINTING").is_none()
        || !crate::fork::is_test_identity(&window.app_handle().config().identifier)
    {
        return;
    }
    let result = window.with_webview(|webview| {
        use objc2::runtime::{AnyObject, Bool};
        use objc2::{msg_send, sel};
        let ptr = webview.inner();
        if ptr.is_null() {
            return;
        }
        // SAFETY: `inner()` is the live WKWebView of this window, used on
        // the main thread inside `with_webview`; the selector is checked
        // before it is sent.
        unsafe {
            let view = &*(ptr as *const AnyObject);
            let sel = sel!(_setWindowOcclusionDetectionEnabled:);
            let answers: bool = msg_send![view, respondsToSelector: sel];
            if answers {
                let _: () = msg_send![view, _setWindowOcclusionDetectionEnabled: Bool::NO];
                log::info!("fork hub: test: occlusion detection off");
            }
        }
    });
    if let Err(e) = result {
        log::warn!("fork hub: test keep-painting failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAR: f64 = 28.0;

    #[test]
    fn default_size_on_a_large_screen() {
        let s = hub_window_size(BAR, Some((1920.0, 1080.0)));
        assert_eq!(s.size, (1040.0, 748.0));
        assert_eq!(s.min, (720.0, 628.0));
    }

    #[test]
    fn unknown_screen_uses_the_default() {
        assert_eq!(hub_window_size(BAR, None).size, (1040.0, 748.0));
    }

    #[test]
    fn shrinks_to_a_small_screen_but_not_below_the_minimum() {
        // MacBook Air 13" at "More space" is larger; 1024×640 is the
        // smallest scaled mode on current Macs.
        let s = hub_window_size(BAR, Some((1024.0, 640.0)));
        assert_eq!(s.size, (976.0, 628.0));
        let tiny = hub_window_size(BAR, Some((600.0, 400.0)));
        assert_eq!(tiny.size, tiny.min);
    }

    #[test]
    fn traffic_lights_sit_in_the_title_strip() {
        let (x, y) = TRAFFIC_LIGHTS;
        assert!(x > 0.0 && x < 24.0);
        // The buttons' measured centre sits in the middle of the strip.
        assert!((y - TRAFFIC_LIGHTS_CENTRE_OFFSET - TITLE_STRIP_HEIGHT / 2.0).abs() < 0.5);
    }

    #[test]
    fn minimum_is_handys_page_size_plus_the_bar() {
        let s = hub_window_size(0.0, None);
        assert_eq!(s.min, crate::utils::MAIN_WINDOW_SIZE);
        let odd = hub_window_size(f64::NAN, None);
        assert_eq!(odd.min, crate::utils::MAIN_WINDOW_SIZE);
        assert_eq!(
            hub_window_size(-5.0, None).min,
            crate::utils::MAIN_WINDOW_SIZE
        );
    }
}
