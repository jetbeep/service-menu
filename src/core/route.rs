#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    MainMenu,
    KeypadTest,
    LockTests,
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
            Self::LockTests => "Placeholder. Press # to run action, * to go back.",
            Self::BarcodeScannerTest => "Placeholder. Press # to run action, * to go back.",
            Self::ConnectivityInfo => "Placeholder. Press # to run action, * to go back.",
            Self::BatteryInfo => "Placeholder. Press # to run action, * to go back.",
            Self::ScreenColorTest => "Placeholder. Press # to run action, * to go back.",
        }
    }
}
