// SPDX-License-Identifier: GPL-3.0-or-later
//! macOS notifications on `UNUserNotificationCenter` (`objc2-user-notifications`
//! 0.3.2, RS-10 fallback: tauri-plugin-notification has no click or action
//! support on macOS desktop).
//!
//! - [`install`] (at Ready) sets the delegate. It does not touch
//!   authorization, so launching never prompts.
//! - [`Center::deliver`] reads the authorization state; when macOS has not
//!   asked yet it asks now (the first real need, S-18 exit criterion) and
//!   posts once granted. Categories (one per kind with buttons) are
//!   registered right before each post, in the current language.
//! - The delegate routes `didReceiveNotificationResponse` (click, button) and
//!   `openSettingsForNotification` to `fork::notify::on_system_response`, and
//!   presents banners even while the app is active (the app only posts while
//!   no window has focus, so a banner is always wanted).
//!
//! `UNUserNotificationCenter` raises an Objective-C exception in a process
//! without an app bundle (`tauri dev`, `cargo test`), so [`bundled`] gates
//! every call.

use std::sync::OnceLock;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread};
use objc2_foundation::{NSArray, NSBundle, NSError, NSObject, NSObjectProtocol, NSSet, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationAction, UNNotificationActionOptions, UNNotificationCategory,
    UNNotificationCategoryOptions, UNNotificationDefaultActionIdentifier,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use super::policy::NotificationButton;
use super::{NotificationPermission, SystemNotification, SystemResponse, SystemSink};

/// Prefix of request identifiers: `wb.n.<inbox id>`.
pub const REQUEST_PREFIX: &str = "wb.n.";

/// The request identifier of inbox entry `id`.
pub fn request_id(id: i64) -> String {
    format!("{REQUEST_PREFIX}{id}")
}

/// The inbox id in a request identifier.
pub fn parse_request_id(identifier: &str) -> Option<i64> {
    identifier.strip_prefix(REQUEST_PREFIX)?.parse().ok()
}

/// What the delegate makes of a response's action identifier.
pub fn response_for(
    identifier: &str,
    action: &str,
    default_action: &str,
) -> Option<SystemResponse> {
    let id = parse_request_id(identifier)?;
    let button = if action == default_action {
        None
    } else {
        // Dismiss (only sent with `CustomDismissAction`, which we do not set)
        // and unknown ids are ignored.
        Some(NotificationButton::parse(action)?)
    };
    Some(SystemResponse::Activated { id, button })
}

/// `true` when this process runs from an app bundle with an identifier.
pub fn bundled() -> bool {
    static BUNDLED: OnceLock<bool> = OnceLock::new();
    *BUNDLED.get_or_init(|| {
        let bundle = NSBundle::mainBundle();
        let has_id = bundle.bundleIdentifier().is_some();
        let is_app = bundle.bundlePath().to_string().ends_with(".app");
        if !(has_id && is_app) {
            log::info!("notify: not running from an app bundle; macOS notifications are off");
        }
        has_id && is_app
    })
}

type Handler = Box<dyn Fn(SystemResponse) + Send + Sync>;

static HANDLER: OnceLock<Handler> = OnceLock::new();

fn dispatch(response: SystemResponse) {
    if let Some(handler) = HANDLER.get() {
        // Off the delegate's thread: activation reads the database, writes the
        // clipboard and shows windows.
        let _ = std::thread::Builder::new()
            .name("wb-notify-response".into())
            .spawn(move || handler(response));
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; no ivars.
    #[unsafe(super(NSObject))]
    #[name = "WisprBroNotificationDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion_handler
                .call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &block2::DynBlock<dyn Fn()>,
        ) {
            let identifier = response.notification().request().identifier().to_string();
            let action = response.actionIdentifier().to_string();
            // SAFETY: a constant NSString exported by UserNotifications.
            let default_action = unsafe { UNNotificationDefaultActionIdentifier }.to_string();
            match response_for(&identifier, &action, &default_action) {
                Some(r) => dispatch(r),
                None => log::debug!("notify: ignored response {action} for {identifier}"),
            }
            completion_handler.call(());
        }

        #[unsafe(method(userNotificationCenter:openSettingsForNotification:))]
        fn open_settings(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: Option<&UNNotification>,
        ) {
            dispatch(SystemResponse::OpenSettings);
        }
    }
);

impl Delegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }
}

/// Sets the delegate (once). Never requests authorization.
pub fn install(handler: impl Fn(SystemResponse) + Send + Sync + 'static) {
    if !bundled() || HANDLER.set(Box::new(handler)).is_err() {
        return;
    }
    let delegate = Delegate::new();
    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // The center holds its delegate weakly; it lives for the process.
    std::mem::forget(delegate);
    log::info!("notify: macOS notification delegate installed");
}

fn register_categories(center: &UNUserNotificationCenter, n: &SystemNotification) {
    let categories: Vec<Retained<UNNotificationCategory>> = n
        .categories
        .iter()
        .map(|(id, buttons)| {
            let actions: Vec<Retained<UNNotificationAction>> = buttons
                .iter()
                .map(|(button, label)| {
                    // Foreground for buttons that show something; Copy latest
                    // runs in the background.
                    let options = match button {
                        NotificationButton::CopyLatest => UNNotificationActionOptions::empty(),
                        _ => UNNotificationActionOptions::Foreground,
                    };
                    UNNotificationAction::actionWithIdentifier_title_options(
                        &NSString::from_str(button.id()),
                        &NSString::from_str(label),
                        options,
                    )
                })
                .collect();
            UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                &NSString::from_str(id),
                &NSArray::from_retained_slice(&actions),
                &NSArray::new(),
                UNNotificationCategoryOptions::empty(),
            )
        })
        .collect();
    // Replaces the app's whole set, in the current language.
    center.setNotificationCategories(&NSSet::from_retained_slice(&categories));
}

fn add_request(n: SystemNotification) {
    let center = UNUserNotificationCenter::currentNotificationCenter();
    register_categories(&center, &n);
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(&n.title));
    content.setBody(&NSString::from_str(&n.body));
    if let Some(id) = n.category {
        content.setCategoryIdentifier(&NSString::from_str(id));
    }
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&request_id(n.id)),
        &content,
        None,
    );
    let id = n.id;
    let done = RcBlock::new(move |error: *mut NSError| {
        // SAFETY: `error` is nil or a valid NSError for the block's duration.
        match unsafe { error.as_ref() } {
            None => log::info!("notify: macOS notification {id} delivered to the center"),
            Some(e) => log::warn!(
                "notify: macOS refused notification {id}: {}",
                e.localizedDescription()
            ),
        }
    });
    center.addNotificationRequest_withCompletionHandler(&request, Some(&done));
}

fn status_of(settings: &UNNotificationSettings) -> NotificationPermission {
    match settings.authorizationStatus() {
        UNAuthorizationStatus::NotDetermined => NotificationPermission::NotDetermined,
        UNAuthorizationStatus::Denied => NotificationPermission::Denied,
        _ => NotificationPermission::Authorized,
    }
}

fn with_permission(f: impl Fn(NotificationPermission) + 'static) {
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let block = RcBlock::new(move |settings: std::ptr::NonNull<UNNotificationSettings>| {
        // SAFETY: a valid settings object for the block's duration.
        f(status_of(unsafe { settings.as_ref() }));
    });
    center.getNotificationSettingsWithCompletionHandler(&block);
}

/// The real sink.
pub struct Center;

impl SystemSink for Center {
    fn available(&self) -> bool {
        bundled()
    }

    fn deliver(&self, n: SystemNotification) {
        if !bundled() {
            return;
        }
        let pending = std::sync::Mutex::new(Some(n));
        with_permission(move |status| {
            let Some(n) = pending.lock().ok().and_then(|mut p| p.take()) else {
                return;
            };
            match status {
                NotificationPermission::Authorized => add_request(n),
                NotificationPermission::NotDetermined => request_then_add(n),
                NotificationPermission::Denied | NotificationPermission::Unavailable => {
                    log::info!(
                        "notify: macOS notifications are off for this app; entry {} stays in the inbox",
                        n.id
                    )
                }
            }
        });
    }

    fn permission(&self) -> NotificationPermission {
        if !bundled() {
            return NotificationPermission::Unavailable;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        with_permission(move |status| {
            let _ = tx.send(status);
        });
        rx.recv_timeout(std::time::Duration::from_secs(2))
            .unwrap_or(NotificationPermission::NotDetermined)
    }
}

/// Asks macOS (the system prompt), then posts `n` if allowed.
fn request_then_add(n: SystemNotification) {
    log::info!(
        "notify: asking macOS for notification permission (first real need: {})",
        n.kind.as_str()
    );
    let pending = std::sync::Mutex::new(Some(n));
    let block = RcBlock::new(move |granted: Bool, error: *mut NSError| {
        let Some(n) = pending.lock().ok().and_then(|mut p| p.take()) else {
            return;
        };
        if granted.as_bool() {
            log::info!("notify: notification permission granted");
            add_request(n);
        } else {
            // SAFETY: nil or a valid NSError for the block's duration.
            let why = unsafe { error.as_ref() }
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "declined".into());
            log::info!("notify: notification permission not granted ({why})");
        }
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert
                | UNAuthorizationOptions::Sound
                | UNAuthorizationOptions::ProvidesAppNotificationSettings,
            &block,
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ids_round_trip() {
        assert_eq!(request_id(42), "wb.n.42");
        assert_eq!(parse_request_id("wb.n.42"), Some(42));
        assert_eq!(parse_request_id("wb.n.x"), None);
        assert_eq!(parse_request_id("other.42"), None);
    }

    #[test]
    fn responses_map_click_and_buttons() {
        let default = "com.apple.UNNotificationDefaultActionIdentifier";
        assert_eq!(
            response_for("wb.n.7", default, default),
            Some(SystemResponse::Activated {
                id: 7,
                button: None
            })
        );
        assert_eq!(
            response_for("wb.n.7", "copy_latest", default),
            Some(SystemResponse::Activated {
                id: 7,
                button: Some(NotificationButton::CopyLatest)
            })
        );
        assert_eq!(
            response_for("wb.n.7", "open_accessibility", default),
            Some(SystemResponse::Activated {
                id: 7,
                button: Some(NotificationButton::OpenAccessibility)
            })
        );
        assert_eq!(
            response_for(
                "wb.n.7",
                "com.apple.UNNotificationDismissActionIdentifier",
                default
            ),
            None
        );
        assert_eq!(response_for("foreign", default, default), None);
    }

    #[test]
    fn a_test_binary_is_not_bundled() {
        // `cargo test` runs a bare executable: the center must never be used.
        assert!(!bundled());
        assert_eq!(Center.permission(), NotificationPermission::Unavailable);
    }
}
