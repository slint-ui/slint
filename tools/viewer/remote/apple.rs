// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! mDNS announcement on Apple platforms and, on iOS, the app lifecycle observation
//! that keeps the announcement alive across network changes.
//!
//! This pumps the Bonjour socket with its own bounded polling loop on top of the
//! synchronous zeroconf crate instead of using zeroconf-tokio: the viewer only needs
//! the socket pumped until the one registration callback has arrived, and
//! zeroconf-tokio's perpetual event processor aborts the process when polling fails,
//! which is exactly what happens once iOS tears down the daemon connection (airplane
//! mode, network switch) while the app is in the background. See
//! https://github.com/windy1/zeroconf-tokio/issues/15 and
//! https://github.com/slint-ui/slint/issues/12043.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use i_slint_live_preview::remote::Connection;
use zeroconf::prelude::{TEventLoop as _, TMdnsService as _, TTxtRecord as _};

/// How long to wait for the registration result before giving up, so that a wedged
/// daemon cannot stall viewer startup indefinitely. The local daemon normally answers
/// within milliseconds.
const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(10);
/// The `select()` timeout of a single poll iteration. `poll()` returns early when
/// daemon data arrives, so this adds no latency to the registration; it only sets how
/// promptly the deadline above is noticed.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Announce the live-preview service over Bonjour and report the advertised instance
/// name to `connection`. The announcement stays active for as long as the returned
/// service handle is alive: the daemon maintains it without any participation from
/// the app.
///
/// The Bonjour socket is only polled until the registration result is in; afterwards
/// nothing reads it.
pub(super) async fn announce_mdns(connection: &Connection) -> Option<zeroconf::MdnsService> {
    let registration_result: Arc<Mutex<Option<zeroconf::Result<zeroconf::ServiceRegistration>>>> =
        Arc::default();

    let (service, event_loop) = (|| {
        let mut service = zeroconf::MdnsService::new(
            zeroconf::ServiceType::new(
                i_slint_live_preview::protocol::SERVICE_TYPE_NAME,
                i_slint_live_preview::protocol::SERVICE_TYPE_PROTOCOL,
            )?,
            connection.local_port(),
        );
        // Deliberately don't set a name: with a NULL/empty instance name Bonjour
        // substitutes the system default service name, which is the user-assigned
        // device name (e.g. "Simon's iPhone" on iOS, the computer name on macOS).
        // This is the friendly name we want to show in the editor.
        let mut txt = zeroconf::TxtRecord::new();
        txt.insert(
            i_slint_live_preview::protocol::TXT_PROTOCOLS_KEY,
            i_slint_live_preview::protocol::PROTOCOL_SUBPROTOCOL,
        )?;
        txt.insert(
            i_slint_live_preview::protocol::TXT_SLINT_VERSION_KEY,
            i_slint_live_preview::protocol::SLINT_VERSION,
        )?;
        service.set_txt_record(txt);
        service.set_registered_callback(Box::new({
            let registration_result = registration_result.clone();
            move |result, _context| {
                *registration_result.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
            }
        }));
        let event_loop = service.register()?;
        Ok::<_, zeroconf::error::Error>((service, event_loop))
    })()
    .inspect_err(|err| tracing::error!("Failed to initialize mDNS: {err}"))
    .ok()?;

    // The callback is only ever invoked from within poll(), on this blocking thread.
    let registration = tokio::task::spawn_blocking({
        let registration_result = registration_result.clone();
        move || {
            let deadline = Instant::now() + REGISTRATION_TIMEOUT;
            loop {
                event_loop.poll(POLL_INTERVAL)?;
                if let Some(result) =
                    registration_result.lock().unwrap_or_else(|e| e.into_inner()).take()
                {
                    return result;
                }
                if Instant::now() >= deadline {
                    return Err("timed out waiting for the registration result".into());
                }
            }
        }
    })
    .await
    // spawn_blocking only fails if the closure panicked, which in this panic=abort
    // binary would have ended the process already.
    .expect("the mDNS registration task cannot panic");

    match registration {
        Ok(registration) => {
            connection.set_device_name(registration.name().to_owned());
            Some(service)
        }
        Err(err) => {
            tracing::error!("Failed to announce service: {err}");
            None
        }
    }
}

/// Sends [`super::Event::Resumed`] whenever the app returns to the foreground after
/// having been suspended, by observing the `UIApplication` lifecycle notifications.
/// The observers live for the rest of the process: `NSNotificationCenter` holds
/// block-based observers strongly until `removeObserver:`, which is never called.
///
/// Sending only when a suspension preceded the activation makes the notification
/// accompanying app startup a non-event by construction, and it is while suspended
/// that the announcement typically dies (network change tearing down the daemon
/// connection).
#[cfg(target_os = "ios")]
pub(super) fn observe_foregrounding(sender: tokio::sync::mpsc::UnboundedSender<super::Event>) {
    use objc2_foundation::{
        NSNotification, NSNotificationCenter, NSNotificationName, NSOperationQueue,
    };
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, Ordering};

    let center = NSNotificationCenter::defaultCenter();
    let main_queue = NSOperationQueue::mainQueue();
    let suspended = Arc::new(AtomicBool::new(false));

    let add_observer =
        |name: &NSNotificationName, block: block2::RcBlock<dyn Fn(NonNull<NSNotification>)>| {
            // Safety: the observed object is NULL, delivery happens on the main queue,
            // and the blocks are sendable: they only capture an atomic flag and a
            // channel sender.
            let _ = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    None,
                    Some(&main_queue),
                    &block,
                )
            };
        };

    add_observer(unsafe { objc2_ui_kit::UIApplicationWillResignActiveNotification }, {
        let suspended = suspended.clone();
        block2::RcBlock::new(move |_| suspended.store(true, Ordering::Relaxed))
    });
    add_observer(
        unsafe { objc2_ui_kit::UIApplicationDidBecomeActiveNotification },
        block2::RcBlock::new(move |_| {
            if suspended.swap(false, Ordering::Relaxed) {
                let _ = sender.send(super::Event::Resumed);
            }
        }),
    );
}
