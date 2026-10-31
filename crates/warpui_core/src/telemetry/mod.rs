mod event_store;

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use event_store::*;
pub use event_store::{Event, EventPayload};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use serde_json::Value;

lazy_static! {
    static ref TELEMETRY: Mutex<EventStore> = Mutex::new(EventStore::new());
}

/// Production builds never construct analytics events. Tests that still exercise
/// the persist filter opt in for the duration of the test.
static CONSTRUCT_ANALYTICS_EVENTS: AtomicBool = AtomicBool::new(false);

pub fn analytics_event_construction_enabled() -> bool {
    CONSTRUCT_ANALYTICS_EVENTS.load(Ordering::Relaxed)
}

#[cfg(feature = "test-util")]
pub fn set_analytics_event_construction_enabled_for_test(enabled: bool) {
    CONSTRUCT_ANALYTICS_EVENTS.store(enabled, Ordering::Relaxed);
}

#[macro_export]
macro_rules! record_telemetry_from_ctx {
    ($user_id: expr, $anonymous_id: expr, $name:expr, $payload: expr, $contains_ugc: expr, $ctx: expr) => {{
        if $crate::telemetry::analytics_event_construction_enabled() {
            let timestamp = $crate::time::get_current_time();
            let user_id = $user_id;
            let anonymous_id = $anonymous_id;
            let name = $name;
            let payload = $payload;
            let contains_ugc = $contains_ugc;
            $ctx.background_executor()
                .spawn(async move {
                    $crate::telemetry::record_event(
                        user_id,
                        anonymous_id,
                        name,
                        payload,
                        contains_ugc,
                        timestamp,
                    )
                })
                .detach();
        }
    }};
}

#[macro_export]
macro_rules! record_telemetry_on_executor {
    ($user_id: expr, $anonymous_id: expr, $name:expr, $payload: expr, $contains_ugc: expr, $executor: expr) => {{
        if $crate::telemetry::analytics_event_construction_enabled() {
            let timestamp = $crate::time::get_current_time();
            let user_id = $user_id;
            let anonymous_id = $anonymous_id;
            let name = $name;
            let payload = $payload;
            let contains_ugc = $contains_ugc;
            let _ = $executor
                .spawn(async move {
                    $crate::telemetry::record_event(
                        user_id,
                        anonymous_id,
                        name,
                        payload,
                        contains_ugc,
                        timestamp,
                    )
                })
                .detach();
        }
    }};
}

/// Creates a new `Event`, but does not record it. It is up to the caller to determine when, and
/// how, the event should be recorded.
pub fn create_event(
    user_id: Option<String>,
    anonymous_id: String,
    name: Cow<'static, str>,
    payload: Option<Value>,
    contains_ugc: bool,
    timestamp: DateTime<Utc>,
) -> Option<Event> {
    if !analytics_event_construction_enabled() {
        return None;
    }
    let mut telemetry = TELEMETRY.lock();
    Some(telemetry.create_event(
        user_id,
        anonymous_id,
        name,
        payload,
        contains_ugc,
        timestamp,
    ))
}

pub fn record_event(
    user_id: Option<String>,
    anonymous_id: String,
    name: Cow<'static, str>,
    payload: Option<Value>,
    contains_ugc: bool,
    timestamp: DateTime<Utc>,
) {
    if !analytics_event_construction_enabled() {
        return;
    }
    let mut telemetry = TELEMETRY.lock();
    telemetry.record_event(
        user_id,
        anonymous_id,
        name,
        payload,
        contains_ugc,
        timestamp,
    );
}

pub fn record_identify_user_event(user_id: String, anonymous_id: String, timestamp: DateTime<Utc>) {
    if !analytics_event_construction_enabled() {
        return;
    }
    let mut telemetry = TELEMETRY.lock();
    telemetry.record_identify_user_event(user_id, anonymous_id, timestamp);
}

/// Adds a 'App Active' event to the global event queue.  This should only be called in an async
/// context.
pub fn record_app_active_event(
    user_id: Option<String>,
    anonymous_id: String,
    timestamp: DateTime<Utc>,
) {
    if !analytics_event_construction_enabled() {
        return;
    }
    let mut telemetry = TELEMETRY.lock();
    telemetry.record_app_active(user_id, anonymous_id, timestamp);
}

pub fn flush_events() -> Vec<Event> {
    TELEMETRY.lock().events.drain(..).collect()
}
