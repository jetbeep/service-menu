use jetbeep::bus::KeypadKey;

use crate::core::route::Route;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockTarget {
    pub board_id: u32,
    pub lock_id: u32,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Open(Route),
    Back,
    Action,
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
        statuses: u64,
        message: alloc::string::String,
    },
    BarcodeScanned(alloc::string::String),
    Keypad(KeypadKey),
}
