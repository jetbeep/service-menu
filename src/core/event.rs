use jetbeep::bus::KeypadKey;

use crate::core::route::Route;

#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    Open(Route),
    Back,
    Action,
    Tick,
    Keypad(KeypadKey),
}
