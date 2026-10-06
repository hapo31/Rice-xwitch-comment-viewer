use crate::app_events::{AppLogLevel, emit_app_log};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

#[derive(Default)]
pub(crate) struct PendingActivation {
    ready: AtomicBool,
    pending: AtomicBool,
}

pub(crate) fn request_activation(app: &tauri::AppHandle) {
    let state = app.state::<PendingActivation>();
    state.pending.store(true, Ordering::SeqCst);
    activate_if_ready(app, &state);
}

pub(crate) fn mark_ready(app: &tauri::AppHandle) {
    let state = app.state::<PendingActivation>();
    state.ready.store(true, Ordering::SeqCst);
    activate_if_ready(app, &state);
}

fn activate_if_ready(app: &tauri::AppHandle, state: &PendingActivation) {
    if !state.ready.load(Ordering::SeqCst) {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if !state.pending.swap(false, Ordering::SeqCst) {
        return;
    }
    if restore_and_focus(&window) != 0 {
        emit_app_log(
            app,
            AppLogLevel::Warning,
            "Rice は既に起動しています。ウィンドウを表示できない場合はタスクバーから開いてください。",
        );
    }
}

trait ActivationWindow {
    fn show(&self) -> bool;
    fn unminimize(&self) -> bool;
    fn focus(&self) -> bool;
}

impl ActivationWindow for tauri::WebviewWindow {
    fn show(&self) -> bool {
        self.show().is_ok()
    }
    fn unminimize(&self) -> bool {
        self.unminimize().is_ok()
    }
    fn focus(&self) -> bool {
        self.set_focus().is_ok()
    }
}

fn restore_and_focus(window: &impl ActivationWindow) -> usize {
    // Do not short-circuit: focus may still succeed if an earlier operation fails.
    [window.show(), window.unminimize(), window.focus()]
        .into_iter()
        .filter(|success| !success)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeWindow {
        calls: RefCell<Vec<&'static str>>,
        show_ok: bool,
    }
    impl ActivationWindow for FakeWindow {
        fn show(&self) -> bool {
            self.calls.borrow_mut().push("show");
            self.show_ok
        }
        fn unminimize(&self) -> bool {
            self.calls.borrow_mut().push("unminimize");
            true
        }
        fn focus(&self) -> bool {
            self.calls.borrow_mut().push("focus");
            true
        }
    }

    #[test]
    fn duplicate_activation_shows_restores_then_focuses() {
        let window = FakeWindow {
            calls: RefCell::new(vec![]),
            show_ok: true,
        };
        assert_eq!(restore_and_focus(&window), 0);
        assert_eq!(*window.calls.borrow(), ["show", "unminimize", "focus"]);
    }

    #[test]
    fn focus_is_attempted_even_if_show_fails() {
        let window = FakeWindow {
            calls: RefCell::new(vec![]),
            show_ok: false,
        };
        assert_eq!(restore_and_focus(&window), 1);
        assert_eq!(*window.calls.borrow(), ["show", "unminimize", "focus"]);
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests;
