// SPDX-License-Identifier: GPL-3.0-or-later
//! Action registry and Esc registry.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::fork::shortcut::actions::*;

fn counter() -> (Arc<AtomicUsize>, Handler) {
    let n = Arc::new(AtomicUsize::new(0));
    let c = n.clone();
    (
        n,
        Arc::new(move || {
            c.fetch_add(1, Ordering::SeqCst);
        }),
    )
}

#[test]
fn fixed_and_runtime_ids() {
    for id in [
        "dictate",
        "hands_free",
        "cancel",
        "paste_last",
        "copy_last",
        "press_enter",
        "undo_cancel",
    ] {
        assert!(is_action_id(id), "{id}");
    }
    for id in [
        "transform.shout",
        "scratchpad.open",
        "notetaker.start",
        "transform.my-style_2",
    ] {
        assert!(is_action_id(id), "{id}");
    }
    for id in [
        "",
        "transform.",
        "Transform.x",
        "other",
        "transform.UPPER",
        "transform.a b",
    ] {
        assert!(!is_action_id(id), "{id:?}");
    }
    assert!(!is_action_id(&format!("transform.{}", "x".repeat(64))));
}

#[test]
fn register_fire_unregister() {
    let reg = Registry::default();
    let (n, h) = counter();
    assert!(!reg.fire("paste_last"));
    reg.register("paste_last", h).unwrap();
    assert!(reg.handles("paste_last"));
    assert!(reg.fire("paste_last"));
    assert_eq!(n.load(Ordering::SeqCst), 1);
    assert!(reg.unregister("paste_last"));
    assert!(!reg.unregister("paste_last"));
    assert!(!reg.fire("paste_last"));
}

#[test]
fn grammar_actions_and_unknown_ids_are_refused() {
    let reg = Registry::default();
    for id in GRAMMAR_ACTIONS {
        assert!(reg.register(id, counter().1).is_err(), "{id}");
    }
    assert!(reg.register("nope", counter().1).is_err());
}

#[test]
fn a_runtime_handler_can_be_replaced() {
    let reg = Registry::default();
    let (a, ha) = counter();
    let (b, hb) = counter();
    reg.register("transform.shout", ha).unwrap();
    reg.register("transform.shout", hb).unwrap();
    reg.fire("transform.shout");
    assert_eq!((a.load(Ordering::SeqCst), b.load(Ordering::SeqCst)), (0, 1));
}

#[test]
fn esc_observers_run_in_order_and_can_leave() {
    let reg = Registry::default();
    let log = Arc::new(Mutex::new(Vec::new()));
    let l1 = log.clone();
    let t1 = reg.observe_esc(Arc::new(move |_| l1.lock().unwrap().push("card")));
    let l2 = log.clone();
    reg.observe_esc(Arc::new(move |ctx| {
        l2.lock().unwrap().push(if ctx.cancelled_take {
            "toast:take"
        } else {
            "toast"
        })
    }));
    reg.notify_esc(EscContext {
        cancelled_take: false,
    });
    reg.unobserve_esc(t1);
    reg.notify_esc(EscContext {
        cancelled_take: true,
    });
    assert_eq!(*log.lock().unwrap(), vec!["card", "toast", "toast:take"]);
}

#[test]
fn an_observer_may_register_another_without_deadlock() {
    let reg = Registry::default();
    let inner = reg.clone();
    reg.observe_esc(Arc::new(move |_| {
        inner.observe_esc(Arc::new(|_| {}));
    }));
    reg.notify_esc(EscContext {
        cancelled_take: false,
    });
    reg.notify_esc(EscContext {
        cancelled_take: false,
    });
}
