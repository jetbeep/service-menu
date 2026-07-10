use jetbeep_core::bus::{KeypadKey, LockStatuses};

use crate::core::route::Route;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockTarget {
    pub board_id: u32,
    pub lock_id: u32,
    /// `false` when the port has no lock settings (lock disabled in device settings).
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Open(Route),
    Back,
    Action,
    SleepRequested,
    Tick,
    KeypadLayoutLoaded(bool),
    LockTargetsLoaded(alloc::vec::Vec<LockTarget>),
    LockOpenAll,
    LockOpenAllStepResult {
        board_id: u32,
        lock_id: u32,
        ok: bool,
        message: alloc::string::String,
        index: usize,
        total: usize,
    },
    LockOpenAllFinished,
    LockOpenSpecificResult {
        board_id: u32,
        lock_id: u32,
        ok: bool,
        message: alloc::string::String,
    },
    LockStatusesResult {
        board_id: u32,
        ok: bool,
        statuses: LockStatuses,
        message: alloc::string::String,
    },
    BarcodeScanned(alloc::string::String),
    ConnectivityInfoLoaded {
        ok: bool,
        text: alloc::string::String,
    },
    ConnectivityServerCheckLoaded {
        connected: bool,
    },
    BatteryInfoLoaded {
        ok: bool,
        text: alloc::string::String,
    },
    Keypad(KeypadKey),
}
