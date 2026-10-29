// SPDX-License-Identifier: GPL-3.0-or-later
//! Test helpers and tests for the fork plumbing (`cfg(test)` only).
//!
//! The tests build a real Tauri app on `tauri::test::MockRuntime` with the
//! fork plugin, a stand-in for Handy's `.setup()` closure and a stand-in Handy
//! invoke handler, all wired exactly as `lib.rs` wires the real ones. Handy's
//! own handler and managers are typed to `tauri::Wry` and cannot run inside a
//! unit test (a Wry event loop needs the macOS main thread), so the parts that
//! touch Handy's real code are covered by source-level checks of `lib.rs` and
//! the generated `src/bindings.ts`, plus `cargo build` itself (the real
//! `compose_invoke(invoke_handler)` call must type-check against Handy's
//! handler).

use super::commands::{ForkModelStatus, HandyModels};

/// Stand-in for Handy's `ModelManager` (same `Arc<…>` state shape).
#[derive(Debug, Clone, Default)]
pub struct FakeModelManager {
    pub status: Option<ForkModelStatus>,
}

impl HandyModels for FakeModelManager {
    fn model_status(&self) -> ForkModelStatus {
        self.status.clone().unwrap_or(ForkModelStatus {
            known_models: 0,
            downloaded_model_ids: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
    use tauri::webview::InvokeRequest;
    use tauri::{App, Manager, RunEvent, WebviewWindow, WebviewWindowBuilder};
    use tauri_specta::Event;

    use super::FakeModelManager;
    use crate::fork::commands::ForkModelStatus;
    use crate::fork::events::ForkPingEvent;
    use crate::fork::{self, ForkState};
    use crate::CliArgs;

    /// Stand-in for a Handy command (Handy's real ones are `Wry`-only).
    #[tauri::command]
    fn get_current_model() -> String {
        "handy-model".into()
    }

    fn sample_status() -> ForkModelStatus {
        ForkModelStatus {
            known_models: 3,
            downloaded_model_ids: vec!["parakeet-v3".into()],
        }
    }

    /// Builds an app wired like `lib.rs`: fork plugin in the plugin chain,
    /// `CliArgs` managed, a `.setup()` that manages the model manager (like
    /// `initialize_core_logic`, skipped in headless mode) and the composed
    /// invoke handler.
    fn build_app(cli_args: CliArgs) -> App<MockRuntime> {
        let headless = fork::is_headless(&cli_args);
        mock_builder()
            .plugin(fork::init())
            .manage(cli_args)
            .setup(move |app| {
                if !headless {
                    app.manage(Arc::new(FakeModelManager {
                        status: Some(sample_status()),
                    }));
                }
                Ok(())
            })
            .invoke_handler(fork::compose_invoke(tauri::generate_handler![
                get_current_model
            ]))
            .build(mock_context(noop_assets()))
            .expect("failed to build mock app")
    }

    fn main_window(app: &App<MockRuntime>) -> WebviewWindow<MockRuntime> {
        WebviewWindowBuilder::new(app, "main", Default::default())
            .build()
            .expect("failed to build mock window")
    }

    fn invoke(webview: &WebviewWindow<MockRuntime>, cmd: &str) -> Result<Value, Value> {
        get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    /// Runs the mock event loop; `on_ready` receives the window right after
    /// `RunEvent::Ready` (Handy's setup and the plugin's `on_event` have run).
    /// The window is then destroyed, which ends the mock loop.
    fn run_until_ready<T: Send + 'static>(
        app: App<MockRuntime>,
        on_ready: impl Fn(&WebviewWindow<MockRuntime>) -> T + 'static,
    ) -> T {
        let out: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
        let sink = out.clone();
        app.run(move |handle, event| {
            if let RunEvent::Ready = event {
                let window = handle.get_webview_window("main").expect("main window");
                *sink.lock().unwrap() = Some(on_ready(&window));
                window.destroy().unwrap();
            }
        });
        let result = out.lock().unwrap().take();
        result.expect("RunEvent::Ready was never delivered")
    }

    #[test]
    fn model_status_is_not_ready_before_ready_and_handy_state_after() {
        let app = build_app(CliArgs::default());
        let window = main_window(&app);

        // Plugin setup has run (inside build()); Handy's setup has not.
        assert!(app.try_state::<ForkState>().is_some());
        assert!(app.try_state::<Arc<FakeModelManager>>().is_none());
        assert_eq!(
            invoke(&window, "fork_model_status"),
            Err(json!({ "kind": "NotReady" }))
        );

        let after = run_until_ready(app, |w| invoke(w, "fork_model_status"));
        assert_eq!(after, Ok(serde_json::to_value(sample_status()).unwrap()));
    }

    #[test]
    fn handy_command_still_resolves_through_composed_handler() {
        let app = build_app(CliArgs::default());
        let window = main_window(&app);
        assert_eq!(
            invoke(&window, "get_current_model"),
            Ok(json!("handy-model"))
        );
        // An unknown non-fork command is still Handy's to reject.
        assert!(invoke(&window, "no_such_command").is_err());
        // An unknown fork command is rejected by the fork handler.
        assert!(invoke(&window, "fork_no_such_command").is_err());
    }

    #[test]
    fn fork_event_round_trips_through_mount_events() {
        assert_eq!(ForkPingEvent::NAME, "fork-ping-event");

        let app = build_app(CliArgs::default());
        let window = main_window(&app);

        // Mount a Handy event into the same app, the way Handy's own builder
        // does in its setup closure, to prove both builders share one registry.
        tauri_specta::Builder::<MockRuntime>::new()
            .events(tauri_specta::collect_events![
                crate::transcription_coordinator::RecordingStateEvent
            ])
            .mount_events(&app);
        // `listen` panics if the event is not in the registry.
        crate::transcription_coordinator::RecordingStateEvent::listen(&app, |_| {});

        let (tx, rx) = mpsc::channel();
        ForkPingEvent::listen(&app, move |event| {
            let _ = tx.send(event.payload);
        });

        let reply = invoke(&window, "fork_ping").expect("fork_ping failed");
        assert_eq!(reply["message"], json!("pong"));
        assert_eq!(reply["seq"], json!(1));
        assert_eq!(reply["ready"], json!(false));

        let received = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("ForkPingEvent was not received");
        assert_eq!(received, ForkPingEvent { seq: 1 });
    }

    #[test]
    fn headless_cli_mode_skips_on_ready_and_opens_no_windows() {
        for args in [
            CliArgs {
                list_models: true,
                ..Default::default()
            },
            CliArgs {
                list_devices: true,
                ..Default::default()
            },
            CliArgs {
                transcribe_file: Some("x.wav".into()),
                ..Default::default()
            },
        ] {
            assert!(fork::is_headless(&args));
            let app = build_app(args);
            let window = main_window(&app);
            let (status, ready, windows) = run_until_ready(app, |w| {
                let handle = w.app_handle();
                (
                    invoke(w, "fork_model_status"),
                    handle.state::<ForkState>().is_ready(),
                    handle.webview_windows().keys().cloned().collect::<Vec<_>>(),
                )
            });
            drop(window);
            assert_eq!(status, Err(json!({ "kind": "NotReady" })));
            assert!(!ready);
            assert_eq!(windows, vec!["main".to_string()]);
        }
        assert!(!fork::is_headless(&CliArgs::default()));
    }

    fn repo_file(rel: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
    }

    /// Command names in Handy's `collect_commands![…]` list in `lib.rs`.
    fn handy_command_names() -> Vec<String> {
        let lib = repo_file("src/lib.rs");
        let start = lib
            .find("collect_commands![")
            .expect("collect_commands! not found in lib.rs")
            + "collect_commands![".len();
        let end = start + lib[start..].find(']').expect("unterminated list");
        lib[start..end]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|path| path.rsplit("::").next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn no_handy_command_name_starts_with_fork_prefix() {
        let names = handy_command_names();
        assert!(names.len() > 50, "parsed only {} commands", names.len());
        let clashes: Vec<_> = names.iter().filter(|n| fork::is_fork_command(n)).collect();
        assert!(clashes.is_empty(), "Handy commands use fork_: {clashes:?}");

        // The generated Handy bindings agree.
        let bindings = repo_file("../src/bindings.ts");
        assert!(bindings.contains("TAURI_INVOKE(\"get_app_settings\""));
        assert!(!bindings.contains("TAURI_INVOKE(\"fork_"));
        assert!(!bindings.contains("\"fork-"));
    }

    #[test]
    fn compose_invoke_is_the_only_invoke_handler() {
        let lib = repo_file("src/lib.rs");
        // `.invoke_handler(` calls on the Tauri builder; `specta_builder.invoke_handler()`
        // (which only creates Handy's handler) is not one.
        let calls =
            lib.matches(".invoke_handler(").count() - lib.matches(".invoke_handler()").count();
        assert_eq!(calls, 1);
        assert!(lib.contains(".invoke_handler(fork::compose_invoke(invoke_handler))"));
        assert!(lib.contains(".plugin(fork::init())"));
    }

    #[test]
    fn build_rs_still_ends_in_plain_tauri_build() {
        let build = repo_file("build.rs");
        assert!(build.contains("    tauri_build::build()\n}"));
        assert!(!build.contains("AppManifest"));
    }

    #[test]
    fn fork_bindings_are_current_and_handy_bindings_untouched() {
        let handy_before = repo_file("../src/bindings.ts");

        let fork_builder = fork::specta_builder::<MockRuntime>();
        let generated = fork_builder
            .export_str(fork::bindings_language())
            .expect("export fork bindings");
        // Export to a scratch file too, exactly like `compose_invoke` does.
        let dir = tempfile::tempdir().unwrap();
        fork_builder
            .export(fork::bindings_language(), dir.path().join("bindings.ts"))
            .expect("export fork bindings to file");

        assert_ne!(fork::FORK_BINDINGS_PATH, "../src/bindings.ts");
        assert_eq!(
            repo_file("../src/bindings.ts"),
            handy_before,
            "fork codegen changed Handy's src/bindings.ts"
        );
        assert!(generated.contains("TAURI_INVOKE(\"fork_ping\""));
        assert!(generated.contains("TAURI_INVOKE(\"fork_model_status\""));
        assert!(generated.contains("\"fork-ping-event\""));
        assert!(!generated.contains("get_app_settings"));

        if std::env::var_os("WB_WRITE_FORK_BINDINGS").is_some() {
            let path =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(fork::FORK_BINDINGS_PATH);
            std::fs::write(path, &generated).unwrap();
        }
        assert_eq!(
            repo_file("../src/fork/bindings.ts"),
            generated,
            "src/fork/bindings.ts is stale: run a debug build (`bun run wb:dev`) or this test with WB_WRITE_FORK_BINDINGS=1"
        );
    }
}
