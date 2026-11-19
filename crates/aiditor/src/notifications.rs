//! System notifications through `UNUserNotificationCenter`, trimmed from Zed's
//! `gpui_macos/src/system_notifications.rs`: no actions, no dismissing, and responses go to the
//! core as messages on the services channel.

use std::sync::atomic::{AtomicBool, Ordering};

use aiditor_core::Msg;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSBundle, NSError, NSObject, NSObjectProtocol, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};
use tokio::sync::mpsc::Sender;

/// Set once a failed post has been noted, so a broken setup notes it once per run.
static POST_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) struct Notifications {
    center: Retained<UNUserNotificationCenter>,
    /// The center's delegate is weak; this keeps click handling alive.
    _delegate: Retained<Delegate>,
}

impl Notifications {
    /// The notification center, asking for authorization, or `None` outside an app bundle, noted
    /// in `*Messages*`.
    pub(crate) fn new(tx: Sender<Msg>) -> Option<Self> {
        // The center aborts the process ("bundleProxyForCurrentProcess is nil") without a bundle.
        if NSBundle::mainBundle().bundleIdentifier().is_none() {
            _ = tx.try_send(Msg::Log("Notifications need aiditor.app".into()));
            return None;
        }
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let log = tx.clone();
        let completion = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            // SAFETY: when non-null, `error` is a valid `NSError` for the callback.
            let line = match unsafe { error.as_ref() } {
                Some(error) => format!("Notifications: {}", error.localizedDescription()),
                None if !granted.as_bool() => "Notifications are not allowed for aiditor".into(),
                None => return,
            };
            _ = log.try_send(Msg::Log(line));
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &completion,
        );
        let delegate = Delegate::new(tx);
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Some(Self {
            center,
            _delegate: delegate,
        })
    }

    /// Posts at once; a newer notification with the same tag replaces the older one.
    pub(crate) fn show(&self, tag: &str, title: &str, body: &str, tx: Sender<Msg>) {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(tag),
            &content,
            None,
        );
        let completion = RcBlock::new(move |error: *mut NSError| {
            // SAFETY: when non-null, `error` is a valid `NSError` for the callback.
            if let Some(error) = unsafe { error.as_ref() }
                && !POST_FAILED.swap(true, Ordering::Relaxed)
            {
                let line = format!("Notification not posted: {}", error.localizedDescription());
                _ = tx.try_send(Msg::Log(line));
            }
        });
        self.center
            .addNotificationRequest_withCompletionHandler(&request, Some(&completion));
    }
}

struct Ivars {
    tx: Sender<Msg>,
}

define_class!(
    // SAFETY: `NSObject` has no subclassing requirements and `Delegate` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[ivars = Ivars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        // Called, possibly off the main thread, when a delivered notification is clicked.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive_notification_response(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &block2::DynBlock<dyn Fn()>,
        ) {
            let tag = response.notification().request().identifier().to_string();
            _ = self.ivars().tx.try_send(Msg::NotificationClicked { tag });
            completion_handler.call(());
        }

        // Without this macOS hides banners while aiditor is frontmost; the core decides when to post.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present_notification(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion_handler
                .call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List,));
        }
    }
);

impl Delegate {
    fn new(tx: Sender<Msg>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Ivars { tx });
        // SAFETY: `NSObject`'s `init` is its designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}
