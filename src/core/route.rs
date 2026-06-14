#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    MainMenu,
    KeypadTest,
    LockTests,
    LockTestsOpenAll,
    LockTestsOpenSpecificBoard,
    LockTestsOpenSpecificLock,
    LockTestsGetStatuses,
    LockTestsGetStatusesResult,
    BarcodeScannerTest,
    ConnectivityInfo,
    BatteryInfo,
    ScreenColorTest,
}

impl Route {
    pub fn from_menu_digit(digit: u8) -> Option<Self> {
        match digit {
            1 => Some(Self::KeypadTest),
            2 => Some(Self::LockTests),
            3 => Some(Self::BarcodeScannerTest),
            4 => Some(Self::ConnectivityInfo),
            5 => Some(Self::BatteryInfo),
            6 => Some(Self::ScreenColorTest),
            _ => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::MainMenu => "Service Menu",
            Self::KeypadTest => "Keypad Test",
            Self::LockTests => "Lock Tests",
            Self::LockTestsOpenAll => "Open All Locks",
            Self::LockTestsOpenSpecificBoard => "Open Specific Lock",
            Self::LockTestsOpenSpecificLock => "Open Specific Lock",
            Self::LockTestsGetStatuses => "Get Lock Statuses Of Board",
            Self::LockTestsGetStatusesResult => "Lock Statuses Result",
            Self::BarcodeScannerTest => "Barcode Scanner Test",
            Self::ConnectivityInfo => "Connectivity Info",
            Self::BatteryInfo => "Battery Info",
            Self::ScreenColorTest => "Screen Color Test",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::MainMenu => "Choose a test by touch or keypad (1-6).",
            Self::KeypadTest => "Press each keypad button at least once. Press # to finish and exit.",
            Self::LockTests => "Choose lock test by touch, or keypad 1-3.",
            Self::LockTestsOpenAll => "Opens all locks known in settings and shows per-lock result.",
            Self::LockTestsOpenSpecificBoard => "Step 1: enter board_id.",
            Self::LockTestsOpenSpecificLock => "Step 2: enter lock_id, then open lock.",
            Self::LockTestsGetStatuses => "Enter board_id, request statuses, and view lock states.",
            Self::LockTestsGetStatusesResult => "Result of lock statuses request.",
            Self::BarcodeScannerTest => "Placeholder. Press # to run action, * to go back.",
            Self::ConnectivityInfo => "Placeholder. Press # to run action, * to go back.",
            Self::BatteryInfo => "Placeholder. Press # to run action, * to go back.",
            Self::ScreenColorTest => "Placeholder. Press # to run action, * to go back.",
        }
    }
}
