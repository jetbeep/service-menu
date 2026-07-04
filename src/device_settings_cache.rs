use ::core::time::Duration;

use jetbeep_core::proto::DeviceSettings;
use jetbeep_core::workq;

enum CacheState {
    Uninitialized,
    Loading,
    Ready(DeviceSettings),
    Failed,
}

struct SyncCell(core::cell::UnsafeCell<CacheState>);

// SAFETY: service-menu app is single-threaded on LVGL/workq loop.
unsafe impl Sync for SyncCell {}

static DEVICE_SETTINGS_CACHE: SyncCell =
    SyncCell(core::cell::UnsafeCell::new(CacheState::Uninitialized));

pub fn begin_load_once() -> bool {
    let state = unsafe { &mut *DEVICE_SETTINGS_CACHE.0.get() };
    match state {
        CacheState::Uninitialized => {
            *state = CacheState::Loading;
            true
        }
        _ => false,
    }
}

pub fn set_loaded(settings: DeviceSettings) {
    let state = unsafe { &mut *DEVICE_SETTINGS_CACHE.0.get() };
    *state = CacheState::Ready(settings);
}

pub fn set_failed() {
    let state = unsafe { &mut *DEVICE_SETTINGS_CACHE.0.get() };
    *state = CacheState::Failed;
}

/// Soft-kill teardown hook: a load in flight when the app was killed will
/// never complete (its completion is generation-gated), so a stuck `Loading`
/// or a transient `Failed` state must not survive into the next launch.
/// Successfully loaded settings stay cached.
pub fn reset_if_not_loaded() {
    let state = unsafe { &mut *DEVICE_SETTINGS_CACHE.0.get() };
    match state {
        CacheState::Ready(_) => {}
        _ => *state = CacheState::Uninitialized,
    }
}

pub fn get_cached() -> Option<DeviceSettings> {
    let state = unsafe { &*DEVICE_SETTINGS_CACHE.0.get() };
    match state {
        CacheState::Ready(settings) => Some(settings.clone()),
        _ => None,
    }
}

pub async fn wait_loaded() -> Option<DeviceSettings> {
    loop {
        let state = unsafe { &*DEVICE_SETTINGS_CACHE.0.get() };
        match state {
            CacheState::Ready(settings) => return Some(settings.clone()),
            CacheState::Failed => return None,
            CacheState::Uninitialized | CacheState::Loading => {
                workq::delay(Duration::from_millis(50)).await;
            }
        }
    }
}