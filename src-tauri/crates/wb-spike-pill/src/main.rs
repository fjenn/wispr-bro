// SPDX-License-Identifier: GPL-3.0-or-later
//
// wb-spike-pill: RS-03 pill panel spike (R5 §7). One process per scenario:
//
//   wb-spike-pill <scenario> --out DIR [--prevent] [--protected] [--n N]
//                 [--hours H] [--takes T]
//
// scenarios: click, hover, spaces, capture, displays, idle, show (just show
// the pill until killed). The app is an Accessory app (no Dock icon), like
// Wispr Bro at rest. Results go to DIR/<scenario>*.json.

mod cg;
mod drive;
mod pill;

use std::path::PathBuf;

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scenario = args.get(1).cloned().unwrap_or_else(|| "show".into());
    let out = PathBuf::from(arg_value(&args, "--out").unwrap_or_else(|| ".".into()));
    let _ = std::fs::create_dir_all(&out);
    let n: usize = arg_value(&args, "--n")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let hours: f64 = arg_value(&args, "--hours")
        .and_then(|v| v.parse().ok())
        .unwrap_or(8.0);
    let takes: usize = arg_value(&args, "--takes")
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let opts = pill::Options {
        prevent_activation: args.iter().any(|a| a == "--prevent"),
        content_protected: args.iter().any(|a| a == "--protected"),
    };
    let alternate = args.iter().any(|a| a == "--alternate");
    pill::now_ms();
    if let Ok(mut b) = drive::WEBKIT_BEFORE.lock() {
        *b = drive::webkit_pids();
    }

    let result = tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![pill::pill_event])
        .setup(move |app| {
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            pill::create(&handle, &opts)?;
            let prevent = opts.prevent_activation;
            let args_alt = alternate;
            let protected = opts.content_protected;
            std::thread::spawn(move || {
                // Let the webview load and the pill settle.
                std::thread::sleep(std::time::Duration::from_millis(2500));
                let variant = if args_alt {
                    "alternate"
                } else if prevent {
                    "prevent"
                } else {
                    "plain"
                };
                let v = match scenario.as_str() {
                    "click" => drive::click(&handle, &out, variant, n),
                    "hover" => drive::hover(&handle, &out, n),
                    "spaces" => drive::spaces(&handle, &out, n),
                    "capture" => drive::capture(&handle, &out, protected),
                    "displays" => drive::displays(&handle, &out, n),
                    "idle" => drive::idle(&handle, &out, hours, takes),
                    _ => loop {
                        std::thread::sleep(std::time::Duration::from_secs(3600));
                    },
                };
                let summary = v.get("summary").cloned().unwrap_or(v);
                println!("{scenario}: {summary}");
                handle.exit(0);
            });
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("wb-spike-pill: {e}");
        std::process::exit(1);
    }
}
