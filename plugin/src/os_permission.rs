//! The operating system's microphone permission, asked only from a user tap.
//!
//! - macOS: AVFoundation (`AVCaptureDevice` authorization for audio). The first
//!   request shows the system prompt; it needs `NSMicrophoneUsageDescription`
//!   in the app's Info.plist. Without that key macOS would terminate the app on
//!   request, so the adapter refuses instead of asking.
//! - Windows: desktop (unpackaged) apps have no per-app runtime prompt API. The
//!   adapter reads the Settings › Privacy › Microphone consent switches
//!   (device-wide, "Microphone access", and "Let desktop apps access your
//!   microphone") and refuses when any of them is set to Deny.
//! - Linux: there is no OS-level microphone permission for unsandboxed apps;
//!   the answer is "granted" and the WebKitGTK media decision still applies.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Granted,
    Denied,
    NotDetermined,
}

#[cfg(target_os = "macos")]
mod imp {
    use super::Status;
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::NSString;
    use std::sync::Mutex;

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    // AVMediaTypeAudio's value.
    const AUDIO: &str = "soun";

    fn capture_device() -> Option<&'static AnyClass> {
        AnyClass::get(c"AVCaptureDevice")
    }

    /// The app declares why it records; asking without it would kill the process.
    pub fn has_usage_description() -> bool {
        let Some(bundle_class) = AnyClass::get(c"NSBundle") else { return false };
        let key = NSString::from_str("NSMicrophoneUsageDescription");
        unsafe {
            let bundle: *mut AnyObject = msg_send![bundle_class, mainBundle];
            if bundle.is_null() {
                return false;
            }
            let value: *mut AnyObject = msg_send![bundle, objectForInfoDictionaryKey: &*key];
            !value.is_null()
        }
    }

    pub fn status() -> Status {
        let Some(class) = capture_device() else { return Status::Denied };
        let media = NSString::from_str(AUDIO);
        // AVAuthorizationStatus: 0 notDetermined, 1 restricted, 2 denied, 3 authorized.
        let raw: isize = unsafe { msg_send![class, authorizationStatusForMediaType: &*media] };
        match raw {
            3 => Status::Granted,
            0 => Status::NotDetermined,
            _ => Status::Denied,
        }
    }

    pub async fn request() -> bool {
        match status() {
            Status::Granted => return true,
            Status::Denied => return false,
            Status::NotDetermined => {}
        }
        if !has_usage_description() {
            return false;
        }
        let Some(class) = capture_device() else { return false };
        // Everything Objective-C stays in this scope: only the receiver crosses the await.
        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
            let tx = Mutex::new(Some(tx));
            let block = RcBlock::new(move |granted: Bool| {
                if let Some(tx) = tx.lock().ok().and_then(|mut slot| slot.take()) {
                    let _ = tx.send(granted.as_bool());
                }
            });
            let media = NSString::from_str(AUDIO);
            unsafe {
                let _: () = msg_send![class, requestAccessForMediaType: &*media, completionHandler: &*block];
            }
            rx
        };
        rx.await.unwrap_or(false)
    }
}

#[cfg(windows)]
mod imp {
    use super::Status;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    const STORE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

    fn denied(root: RegKey, path: &str) -> bool {
        root.open_subkey(path)
            .and_then(|key| key.get_value::<String, _>("Value"))
            .map(|value| value.eq_ignore_ascii_case("deny"))
            .unwrap_or(false)
    }

    pub fn status() -> Status {
        let non_packaged = format!(r"{STORE}\NonPackaged");
        let machine = || RegKey::predef(HKEY_LOCAL_MACHINE);
        let user = || RegKey::predef(HKEY_CURRENT_USER);
        if denied(machine(), STORE) || denied(user(), STORE) || denied(user(), &non_packaged) {
            Status::Denied
        } else {
            Status::Granted
        }
    }

    pub async fn request() -> bool {
        status() == Status::Granted
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    use super::Status;

    pub fn status() -> Status {
        Status::Granted
    }

    pub async fn request() -> bool {
        true
    }
}

/// The current OS decision, without prompting. Rechecked on every use, so a
/// permission revoked in system settings is seen at the next activation.
pub fn status() -> Status {
    imp::status()
}

/// Ask the OS (a prompt only where the OS shows one). Call ONLY from a user tap.
pub async fn request() -> bool {
    imp::request().await
}
