use alloc::{format, string::String, vec, vec::Vec};
use ::core::time::Duration;

use jetbeep_core::bus::{JkvValue, KeypadKey};
use jetbeep_core::proto::bus::{LockStatus, NetworkRegistrationStatus};
use jetbeep_core::{executor, workq};
use lvgl_dsl::prelude::{Obj, Widget};

use crate::core::{
    app_dispatcher::post_app_event,
    event::{AppEvent, LockTarget},
    nav::Stack,
    route::Route,
};

const MAX_LOCK_LINES: usize = 256;
const AFK_TIMEOUT_SECS: i64 = 60;

pub struct App {
    route_stack: Stack<Route>,
    root: Option<Obj>,
    action_count: u32,
    keypad_has_letters: bool,
    keypad_pressed_mask: u32,
    keypad_last_pressed: Option<KeypadKey>,
    lock_targets: Vec<LockTarget>,
    lock_open_all_running: bool,
    lock_open_all_total: u32,
    lock_open_all_opened: u32,
    lock_open_all_ok: u32,
    lock_open_all_err: u32,
    lock_open_all_lines: Vec<String>,
    lock_open_specific_board_input: String,
    lock_open_specific_lock_input: String,
    lock_open_specific_board_id: Option<u32>,
    lock_open_specific_running: bool,
    lock_open_specific_status: String,
    lock_statuses_board_input: String,
    lock_statuses_running: bool,
    lock_statuses_input_status: String,
    lock_statuses_result_status: String,
    barcode_status: String,
    barcode_last_text: String,
    connectivity_loading: bool,
    connectivity_info_text: String,
    connectivity_server_state: String,
    battery_loading: bool,
    battery_info_text: String,
    last_user_activity_secs: i64,
    sleep_requested: bool,
}

impl App {
    pub fn new() -> Self {
        let mut route_stack = Stack::new();
        route_stack.push(Route::MainMenu);
        let now_secs = Self::current_unix_seconds();
        Self {
            route_stack,
            root: None,
            action_count: 0,
            keypad_has_letters: false,
            keypad_pressed_mask: 0,
            keypad_last_pressed: None,
            lock_targets: Vec::new(),
            lock_open_all_running: false,
            lock_open_all_total: 0,
            lock_open_all_opened: 0,
            lock_open_all_ok: 0,
            lock_open_all_err: 0,
            lock_open_all_lines: Vec::new(),
            lock_open_specific_board_input: String::new(),
            lock_open_specific_lock_input: String::new(),
            lock_open_specific_board_id: None,
            lock_open_specific_running: false,
            lock_open_specific_status: String::from("Enter board_id"),
            lock_statuses_board_input: String::new(),
            lock_statuses_running: false,
            lock_statuses_input_status: String::from("Enter board_id"),
            lock_statuses_result_status: String::from("No result yet"),
            barcode_status: String::from("Scanner stopped"),
            barcode_last_text: String::from("-"),
            connectivity_loading: false,
            connectivity_info_text: String::from("No modem info loaded yet"),
            connectivity_server_state: String::from("Error"),
            battery_loading: false,
            battery_info_text: String::from("No battery info loaded yet"),
            last_user_activity_secs: now_secs,
            sleep_requested: false,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Open(route) => {
                self.mark_user_activity();
                self.reset_submenu_state(route);
                self.route_stack.push(route);

                if route == Route::BarcodeScannerTest {
                    self.start_barcode_scanner();
                }

                if route == Route::ConnectivityInfo {
                    self.start_connectivity_load();
                }

                if route == Route::BatteryInfo {
                    self.start_battery_load();
                }

                self.render();
            }
            AppEvent::Back => {
                self.mark_user_activity();
                if self.route_stack.len() > 1 {
                    if self.current_route() == Route::BarcodeScannerTest {
                        self.stop_barcode_scanner();
                    }
                    self.route_stack.pop();
                    self.render();
                }
            }
            AppEvent::Action => {
                self.mark_user_activity();
                if self.current_route() == Route::ConnectivityInfo {
                    self.start_connectivity_load();
                    self.render();
                } else if self.current_route() == Route::BatteryInfo {
                    self.start_battery_load();
                    self.render();
                } else if self.current_route() != Route::MainMenu {
                    self.action_count = self.action_count.saturating_add(1);
                    self.render();
                }
            }
            AppEvent::SleepRequested => self.request_sleep(),
            AppEvent::Tick => {
                self.handle_afk_tick();
                self.render();
            }
            AppEvent::KeypadLayoutLoaded(has_letters) => {
                self.keypad_has_letters = has_letters;
                self.render();
            }
            AppEvent::LockTargetsLoaded(targets) => {
                self.lock_targets = targets;
                self.lock_open_all_total = self.enabled_lock_count() as u32;
                self.render();
            }
            AppEvent::LockOpenAll => {
                self.mark_user_activity();
                self.start_open_all_locks()
            }
            AppEvent::LockOpenAllStepResult {
                board_id,
                lock_id,
                ok,
                message,
                index,
                total,
            } => {
                // Opening every lock can take longer than the AFK timeout, so
                // treat each step as activity to keep the app awake.
                self.mark_user_activity();
                if ok {
                    self.lock_open_all_ok = self.lock_open_all_ok.saturating_add(1);
                } else {
                    self.lock_open_all_err = self.lock_open_all_err.saturating_add(1);
                }

                self.lock_open_all_opened = (index + 1) as u32;
                self.lock_open_all_total = total as u32;
                self.push_lock_open_all_line(format!(
                    "{}:{} {} ({})",
                    board_id,
                    lock_id,
                    if ok { "OK" } else { "ERROR" },
                    message
                ));
                self.render();
            }
            AppEvent::LockOpenAllFinished => {
                self.mark_user_activity();
                self.lock_open_all_running = false;
                self.push_lock_open_all_line(format!(
                    "Done: {} OK, {} ERROR",
                    self.lock_open_all_ok,
                    self.lock_open_all_err
                ));
                self.render();
            }
            AppEvent::LockOpenSpecificResult {
                board_id,
                lock_id,
                ok,
                message,
            } => {
                self.lock_open_specific_running = false;
                if ok {
                    self.lock_open_specific_lock_input.clear();
                }
                self.lock_open_specific_status = format!(
                    "{}:{} {} ({})",
                    board_id,
                    lock_id,
                    if ok { "OK" } else { "ERROR" },
                    message
                );
                self.render();
            }
            AppEvent::LockStatusesResult {
                board_id,
                ok,
                statuses,
                message,
            } => {
                self.lock_statuses_running = false;
                self.lock_statuses_result_status = if ok {
                    self.format_lock_statuses(board_id, statuses)
                } else {
                    format!("Board {} ERROR ({})", board_id, message)
                };
                self.render();
            }
            AppEvent::BarcodeScanned(barcode) => {
                if self.current_route() == Route::BarcodeScannerTest {
                    self.barcode_last_text = barcode;
                    self.barcode_status = String::from("Scanned. Restarting scanner...");
                    self.restart_barcode_scanner();
                    self.render();
                }
            }
            AppEvent::ConnectivityInfoLoaded { ok, text } => {
                self.connectivity_loading = false;
                self.connectivity_info_text = if ok {
                    text
                } else {
                    format!("Failed to load modem info: {}", text)
                };
                self.render();
            }
            AppEvent::ConnectivityServerCheckLoaded { connected } => {
                self.connectivity_server_state = if connected {
                    String::from("Connected")
                } else {
                    String::from("Error")
                };
                self.render();
            }
            AppEvent::BatteryInfoLoaded { ok, text } => {
                self.battery_loading = false;
                self.battery_info_text = if ok {
                    text
                } else {
                    format!("Failed to load battery info: {}", text)
                };
                self.render();
            }
            AppEvent::Keypad(key) => {
                self.mark_user_activity();
                self.handle_keypad(key)
            }
        }
    }

    pub fn render(&mut self) {
        if let Some(root) = self.root.take() {
            root.delete();
        }
        self.root = Some(crate::ui::render(self));
    }

    /// Delete the whole UI tree (soft-kill path).
    pub fn teardown(&mut self) {
        if let Some(root) = self.root.take() {
            root.delete();
        }
    }

    pub fn current_route(&self) -> Route {
        self.route_stack.last().copied().unwrap_or(Route::MainMenu)
    }

    pub fn action_text(&self) -> String {
        if self.current_route() == Route::KeypadTest {
            return self.keypad_status_text();
        }

        if self.current_route() == Route::MainMenu {
            return String::new();
        }

        let mut msg = String::from("Action placeholder executed: ");
        msg.push_str(self.current_route().title());
        msg.push_str(" (#");
        msg.push_str(alloc::format!("{}", self.action_count).as_str());
        msg.push(')');
        msg
    }

    pub fn keypad_layout_name(&self) -> &'static str {
        if self.keypad_has_letters {
            "4x4"
        } else {
            "3x4"
        }
    }

    pub fn keypad_required_keys_text(&self) -> String {
        let mut keys = vec!["1", "2", "3", "4", "5", "6", "7", "8", "9", "*", "0", "#"];
        if self.keypad_has_letters {
            keys.extend_from_slice(&["A", "B", "C", "D"]);
        }
        format!("Required keys: {}", keys.join(" "))
    }

    pub fn keypad_last_pressed_text(&self) -> String {
        match self.keypad_last_pressed {
            Some(key) => format!("Last key: {}", key.label()),
            None => String::from("Last key: -"),
        }
    }

    pub fn lock_open_all_configured_count(&self) -> usize {
        self.enabled_lock_count()
    }

    fn enabled_lock_count(&self) -> usize {
        self.lock_targets.iter().filter(|t| t.enabled).count()
    }

    pub fn lock_open_all_running(&self) -> bool {
        self.lock_open_all_running
    }

    pub fn lock_open_all_summary_text(&self) -> String {
        if self.lock_open_all_running {
            return format!(
                "Running: {}/{} | {} OK | {} ERROR",
                self.lock_open_all_opened,
                self.lock_open_all_total,
                self.lock_open_all_ok,
                self.lock_open_all_err
            );
        }

        if self.lock_open_all_opened == 0 {
            return String::from("Idle");
        }

        format!(
            "Last run: {}/{} | {} OK | {} ERROR",
            self.lock_open_all_opened,
            self.lock_open_all_total,
            self.lock_open_all_ok,
            self.lock_open_all_err
        )
    }

    pub fn lock_open_all_results_text(&self) -> String {
        if self.lock_open_all_lines.is_empty() {
            return String::from("No results yet");
        }
        self.lock_open_all_lines.join("\n")
    }

    pub fn lock_open_specific_board_input_text(&self) -> String {
        if self.lock_open_specific_board_input.is_empty() {
            return String::from("-");
        }
        self.lock_open_specific_board_input.clone()
    }

    pub fn lock_open_specific_lock_input_text(&self) -> String {
        if self.lock_open_specific_lock_input.is_empty() {
            return String::from("-");
        }
        self.lock_open_specific_lock_input.clone()
    }

    pub fn lock_open_specific_available_boards_text(&self) -> String {
        let mut boards: Vec<u32> = Vec::new();
        for t in &self.lock_targets {
            if !boards.contains(&t.board_id) {
                boards.push(t.board_id);
            }
        }
        boards.sort_unstable();

        if boards.is_empty() {
            return String::from("Available boards: none");
        }

        let mut out = String::from("Available boards: ");
        for (i, board_id) in boards.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(format!("{}", board_id).as_str());
        }
        out
    }

    pub fn lock_open_specific_available_locks_text(&self) -> String {
        let Some(board_id) = self.lock_open_specific_board_id else {
            return String::from("Available locks: select board first");
        };

        let mut locks: Vec<u32> = Vec::new();
        for t in &self.lock_targets {
            if t.board_id == board_id {
                locks.push(t.lock_id);
            }
        }
        locks.sort_unstable();

        if locks.is_empty() {
            return format!("Available locks on board {}: none", board_id);
        }

        let mut out = format!("Available locks on board {}: ", board_id);
        for (i, lock_id) in locks.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(format!("{}", lock_id).as_str());
        }
        out
    }

    pub fn lock_open_specific_status_text(&self) -> String {
        self.lock_open_specific_status.clone()
    }

    pub fn lock_open_specific_running(&self) -> bool {
        self.lock_open_specific_running
    }

    pub fn lock_open_specific_board_has_input(&self) -> bool {
        !self.lock_open_specific_board_input.is_empty()
    }

    pub fn lock_open_specific_lock_has_input(&self) -> bool {
        !self.lock_open_specific_lock_input.is_empty()
    }

    pub fn lock_statuses_board_input_text(&self) -> String {
        if self.lock_statuses_board_input.is_empty() {
            return String::from("-");
        }
        self.lock_statuses_board_input.clone()
    }

    pub fn lock_statuses_board_has_input(&self) -> bool {
        !self.lock_statuses_board_input.is_empty()
    }

    pub fn lock_statuses_running(&self) -> bool {
        self.lock_statuses_running
    }

    pub fn lock_statuses_input_status_text(&self) -> String {
        self.lock_statuses_input_status.clone()
    }

    pub fn lock_statuses_result_status_text(&self) -> String {
        self.lock_statuses_result_status.clone()
    }

    pub fn barcode_status_text(&self) -> String {
        self.barcode_status.clone()
    }

    pub fn barcode_last_text(&self) -> String {
        self.barcode_last_text.clone()
    }

    pub fn connectivity_loading(&self) -> bool {
        self.connectivity_loading
    }

    pub fn connectivity_info_text(&self) -> String {
        self.connectivity_info_text.clone()
    }

    pub fn connectivity_server_state(&self) -> String {
        self.connectivity_server_state.clone()
    }

    pub fn battery_loading(&self) -> bool {
        self.battery_loading
    }

    pub fn battery_info_text(&self) -> String {
        self.battery_info_text.clone()
    }

    pub fn afk_countdown_text(&self) -> String {
        format!("AFK: {}s", self.afk_seconds_left())
    }

    fn keypad_status_text(&self) -> String {
        let required = self.required_keypad_mask();
        if (self.keypad_pressed_mask & required) == required {
            return String::from("Result: PASS");
        }

        let missing = self.keypad_missing_labels();
        if missing.is_empty() {
            String::from("Result: PASS")
        } else {
            format!("Missing: {}", missing.join(" "))
        }
    }

    fn handle_keypad(&mut self, key: KeypadKey) {
        if self.current_route() == Route::KeypadTest {
            self.track_keypad_key(key);
            if matches!(key, KeypadKey::Hash) && self.route_stack.len() > 1 {
                self.route_stack.pop();
            }
            self.render();
            return;
        }

        if self.current_route() == Route::BarcodeScannerTest {
            match key {
                KeypadKey::Star => {
                    if self.route_stack.len() > 1 {
                        self.stop_barcode_scanner();
                        self.route_stack.pop();
                        self.render();
                    }
                }
                KeypadKey::Hash => {
                    self.barcode_status = String::from("Restarting scanner...");
                    self.restart_barcode_scanner();
                    self.render();
                }
                _ => {}
            }
            return;
        }

        if matches!(
            self.current_route(),
            Route::LockTests
                | Route::LockTestsOpenAll
                | Route::LockTestsOpenSpecificBoard
                | Route::LockTestsOpenSpecificLock
                | Route::LockTestsGetStatuses
                | Route::LockTestsGetStatusesResult
        ) {
            self.handle_lock_tests_keypad(key);
            return;
        }

        match self.current_route() {
            Route::MainMenu => {
                match key {
                    KeypadKey::Digit(d) => {
                        if let Some(route) = Route::from_menu_digit(d) {
                            if route == Route::BarcodeScannerTest {
                                self.start_barcode_scanner();
                            }

                            if route == Route::ConnectivityInfo {
                                self.start_connectivity_load();
                            }

                            if route == Route::BatteryInfo {
                                self.start_battery_load();
                            }

                            self.route_stack.push(route);
                            self.render();
                        }
                    }
                    KeypadKey::Hash => self.request_sleep(),
                    _ => {}
                }
            }
            _ => match key {
                KeypadKey::Star => {
                    if self.route_stack.len() > 1 {
                        self.route_stack.pop();
                        self.render();
                    }
                }
                KeypadKey::Hash => {
                    if self.current_route() == Route::ConnectivityInfo {
                        self.start_connectivity_load();
                    } else if self.current_route() == Route::BatteryInfo {
                        self.start_battery_load();
                    } else {
                        self.action_count = self.action_count.saturating_add(1);
                    }
                    self.render();
                }
                _ => {}
            },
        }
    }

    fn handle_lock_tests_keypad(&mut self, key: KeypadKey) {
        match self.current_route() {
            Route::LockTests => match key {
                KeypadKey::Digit(1) => {
                    self.reset_submenu_state(Route::LockTestsOpenAll);
                    self.route_stack.push(Route::LockTestsOpenAll);
                    self.render();
                }
                KeypadKey::Digit(2) => {
                    self.reset_submenu_state(Route::LockTestsOpenSpecificBoard);
                    self.route_stack.push(Route::LockTestsOpenSpecificBoard);
                    self.render();
                }
                KeypadKey::Digit(3) => {
                    self.reset_submenu_state(Route::LockTestsGetStatuses);
                    self.route_stack.push(Route::LockTestsGetStatuses);
                    self.render();
                }
                KeypadKey::Star => {
                    if self.route_stack.len() > 1 {
                        self.route_stack.pop();
                        self.render();
                    }
                }
                _ => {}
            },
            Route::LockTestsOpenAll => match key {
                KeypadKey::Hash => self.start_open_all_locks(),
                KeypadKey::Star => {
                    if self.route_stack.len() > 1 {
                        self.route_stack.pop();
                        self.render();
                    }
                }
                _ => {}
            },
            Route::LockTestsOpenSpecificBoard => self.handle_open_specific_board_keypad(key),
            Route::LockTestsOpenSpecificLock => self.handle_open_specific_lock_keypad(key),
            Route::LockTestsGetStatuses => self.handle_get_statuses_keypad(key),
            Route::LockTestsGetStatusesResult => match key {
                KeypadKey::Star => {
                    if self.route_stack.len() > 1 {
                        self.route_stack.pop();
                        self.render();
                    }
                }
                KeypadKey::Hash => self.exit_to_main_menu(),
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_get_statuses_keypad(&mut self, key: KeypadKey) {
        match key {
            KeypadKey::Digit(d) => {
                if self.lock_statuses_board_input.len() < 5 {
                    self.lock_statuses_board_input.push(char::from(b'0' + d));
                }
                self.lock_statuses_input_status = String::from("Press # to get statuses");
                self.render();
            }
            KeypadKey::Star => {
                if !self.lock_statuses_board_input.is_empty() {
                    self.lock_statuses_board_input.pop();
                    self.lock_statuses_input_status = String::from("Press # to get statuses");
                    self.render();
                } else if self.route_stack.len() > 1 {
                    self.route_stack.pop();
                    self.render();
                }
            }
            KeypadKey::Hash => self.submit_get_statuses(),
            _ => {}
        }
    }

    fn handle_open_specific_board_keypad(&mut self, key: KeypadKey) {
        match key {
            KeypadKey::Digit(d) => {
                if self.lock_open_specific_board_input.len() < 5 {
                    self.lock_open_specific_board_input.push(char::from(b'0' + d));
                }
                self.lock_open_specific_status = String::from("Press # to continue");
                self.render();
            }
            KeypadKey::Star => {
                if !self.lock_open_specific_board_input.is_empty() {
                    self.lock_open_specific_board_input.pop();
                    self.lock_open_specific_status = String::from("Press # to continue");
                    self.render();
                } else if self.route_stack.len() > 1 {
                    self.route_stack.pop();
                    self.render();
                }
            }
            KeypadKey::Hash => self.submit_open_specific_board(),
            _ => {}
        }
    }

    fn handle_open_specific_lock_keypad(&mut self, key: KeypadKey) {
        match key {
            KeypadKey::Digit(d) => {
                if self.lock_open_specific_lock_input.len() < 5 {
                    self.lock_open_specific_lock_input.push(char::from(b'0' + d));
                }
                self.lock_open_specific_status = String::from("Press # to open lock");
                self.render();
            }
            KeypadKey::Star => {
                if !self.lock_open_specific_lock_input.is_empty() {
                    self.lock_open_specific_lock_input.pop();
                    self.lock_open_specific_status = String::from("Press # to open lock");
                    self.render();
                } else if self.route_stack.len() > 1 {
                    self.route_stack.pop();
                    self.render();
                }
            }
            KeypadKey::Hash => {
                if self.lock_open_specific_lock_input.is_empty() {
                    self.exit_to_main_menu();
                } else {
                    self.submit_open_specific_lock();
                }
            }
            _ => {}
        }
    }

    fn exit_to_main_menu(&mut self) {
        while self.route_stack.len() > 1 {
            self.route_stack.pop();
        }
        self.render();
    }

    fn submit_open_specific_board(&mut self) {
        if self.lock_open_specific_board_input.is_empty() {
            self.lock_open_specific_status = String::from("Enter board_id first");
            self.render();
            return;
        }

        let Ok(board_id) = self.lock_open_specific_board_input.parse::<u32>() else {
            self.lock_open_specific_status = String::from("Invalid board_id");
            self.render();
            return;
        };

        if !self.lock_targets.iter().any(|t| t.board_id == board_id) {
            self.lock_open_specific_status = format!("Board {} not found in settings", board_id);
            self.render();
            return;
        }

        self.lock_open_specific_board_id = Some(board_id);
        self.lock_open_specific_lock_input.clear();
        self.lock_open_specific_running = false;
        self.lock_open_specific_status = format!("Board {} selected. Enter lock_id.", board_id);
        self.route_stack.push(Route::LockTestsOpenSpecificLock);
        self.render();
    }

    fn submit_open_specific_lock(&mut self) {
        if self.lock_open_specific_running {
            return;
        }

        let Some(board_id) = self.lock_open_specific_board_id else {
            self.lock_open_specific_status = String::from("Select board_id first");
            self.render();
            return;
        };

        if self.lock_open_specific_lock_input.is_empty() {
            self.lock_open_specific_status = String::from("Enter lock_id first");
            self.render();
            return;
        }

        let Ok(lock_id) = self.lock_open_specific_lock_input.parse::<u32>() else {
            self.lock_open_specific_status = String::from("Invalid lock_id");
            self.render();
            return;
        };

        if !self
            .lock_targets
            .iter()
            .any(|t| t.board_id == board_id && t.lock_id == lock_id)
        {
            self.lock_open_specific_status =
                format!("Lock {} not found on board {}", lock_id, board_id);
            self.render();
            return;
        }

        self.lock_open_specific_running = true;
        self.lock_open_specific_status = format!("Opening {}:{}...", board_id, lock_id);
        self.render();

        executor::run(async move {
            let result = jetbeep_core::bus::lock_open(board_id, lock_id).await;
            let (ok, message) = match result {
                Ok(()) => (true, String::from("opened")),
                Err(err) => (false, format!("{}", err)),
            };

            post_app_event(AppEvent::LockOpenSpecificResult {
                board_id,
                lock_id,
                ok,
                message,
            });
        });
    }

    fn submit_get_statuses(&mut self) {
        if self.lock_statuses_running {
            return;
        }

        if self.lock_statuses_board_input.is_empty() {
            self.lock_statuses_input_status = String::from("Enter board_id first");
            self.render();
            return;
        }

        let Ok(board_id) = self.lock_statuses_board_input.parse::<u32>() else {
            self.lock_statuses_input_status = String::from("Invalid board_id");
            self.render();
            return;
        };

        if !self.lock_targets.iter().any(|t| t.board_id == board_id) {
            self.lock_statuses_input_status = format!("Board {} not found in settings", board_id);
            self.render();
            return;
        }

        self.lock_statuses_running = true;
        self.lock_statuses_input_status = format!("Reading statuses for board {}...", board_id);
        self.lock_statuses_result_status = format!("Reading statuses for board {}...", board_id);
        self.route_stack.push(Route::LockTestsGetStatusesResult);
        self.render();

        executor::run(async move {
            let result = jetbeep_core::bus::lock_statuses_get(board_id).await;
            let (ok, statuses, message) = match result {
                Ok(statuses) => (true, statuses, String::new()),
                Err(err) => (false, Vec::new(), format!("{}", err)),
            };

            post_app_event(AppEvent::LockStatusesResult {
                board_id,
                ok,
                statuses,
                message,
            });
        });
    }

    fn format_lock_statuses(&self, board_id: u32, statuses: Vec<LockStatus>) -> String {
        let mut locks: Vec<u32> = self
            .lock_targets
            .iter()
            .filter(|t| t.board_id == board_id)
            .map(|t| t.lock_id)
            .collect();
        locks.sort_unstable();

        let mut closed = 0u32;
        let mut open = 0u32;
        let mut disabled = 0u32;
        let mut lines: Vec<String> = Vec::new();

        for lock_id in locks {
            let state = statuses.get(lock_id as usize).copied().unwrap_or(LockStatus::Disabled);

            let state_label = match state {
                LockStatus::Closed => {
                    closed = closed.saturating_add(1);
                    "CLOSED"
                }
                LockStatus::Opened => {
                    open = open.saturating_add(1);
                    "OPEN"
                }
                LockStatus::Disabled => {
                    disabled = disabled.saturating_add(1);
                    "DISABLED"
                }
            };

            lines.push(format!(
                "{}:{} {}",
                board_id,
                lock_id,
                state_label
            ));
        }

        let mut out = format!(
            "Board {} | {} CLOSED | {} OPEN | {} DISABLED",
            board_id,
            closed,
            open,
            disabled
        );

        if !lines.is_empty() {
            out.push('\n');
            out.push_str(lines.join("\n").as_str());
        }

        out
    }

    fn start_barcode_scanner(&mut self) {
        self.barcode_status = String::from("Waiting for scan...");
        executor::run(async {
            if let Err(err) = jetbeep_core::bus::barcode_scanner_start().await {
                log::warn!("barcode_scanner_start failed: {}", err);
            }
        });
    }

    fn stop_barcode_scanner(&mut self) {
        self.barcode_status = String::from("Scanner stopped");
        executor::run(async {
            if let Err(err) = jetbeep_core::bus::barcode_scanner_stop().await {
                log::warn!("barcode_scanner_stop failed: {}", err);
            }
        });
    }

    fn restart_barcode_scanner(&mut self) {
        self.barcode_status = String::from("Waiting for next scan...");
        executor::run(async {
            if let Err(err) = jetbeep_core::bus::barcode_scanner_stop().await {
                log::warn!("barcode_scanner_stop failed: {}", err);
            }
            if let Err(err) = jetbeep_core::bus::barcode_scanner_start().await {
                log::warn!("barcode_scanner_start failed: {}", err);
            }
        });
    }

    fn start_connectivity_load(&mut self) {
        if self.connectivity_loading {
            return;
        }

        self.connectivity_loading = true;
        self.connectivity_info_text = String::from("Loading modem info...");
        self.connectivity_server_state = String::from("In progress");

        executor::run(async {
            let event = match jetbeep_core::bus::modem_get_info().await {
                Ok(info) => AppEvent::ConnectivityInfoLoaded {
                    ok: true,
                    text: App::format_connectivity_info(&info),
                },
                Err(err) => AppEvent::ConnectivityInfoLoaded {
                    ok: false,
                    text: format!("{}", err),
                },
            };

            post_app_event(event);
        });

        executor::run(async {
            let connected = jetbeep_core::bus::server_request_ex(
                JkvValue::Collection(vec![]),
                jetbeep_core::bus::ServerRequestParams {
                    request_type: 104,
                    timeout_ms: 10_000,
                    ..Default::default()
                },
            )
            .await
            .is_ok();
            post_app_event(AppEvent::ConnectivityServerCheckLoaded { connected });
        });
    }

    fn format_connectivity_info(info: &jetbeep_core::bus::ModemInfo) -> String {
        let mut lines: Vec<String> = Vec::new();

        if let Some(modem) = info.modem_info.as_ref() {
            lines.push(format!("IMEI: {}", modem.imei));
        } else {
            lines.push(String::from("IMEI: -"));
        }

        if let Some(sim) = info.sim_info.as_ref() {
            lines.push(format!("SIM initialized: {}", sim.initialized));
            lines.push(format!("IMSI: {}", sim.imsi));
            lines.push(format!("ICCID: {}", sim.iccid));
        } else {
            lines.push(String::from("SIM: -"));
        }

        if let Some(net) = info.network_reg_info.as_ref() {
            let status = NetworkRegistrationStatus::try_from(net.status)
                .map(|v| format!("{:?}", v))
                .unwrap_or_else(|_| format!("{}", net.status));
            lines.push(format!("Network status: {}", status));
            lines.push(format!("Operator: {} ({})", net.operator_name, net.mcc_mnc));
            lines.push(format!("Cell: TAC={} CID={}", net.tracking_area_code, net.cell_id));
            lines.push(format!("Band: {}", net.band));
            lines.push(format!("PDP active: {}", net.pdp_activated));
            lines.push(format!("IP: {}", net.ip_address));
        } else {
            lines.push(String::from("Network: -"));
        }

        if let Some(sig) = info.signal_info.as_ref() {
            lines.push(format!("Signal RSPR: {} (idx {})", sig.rspr, sig.rspr_index));
            lines.push(format!("Signal RSRQ: {} (idx {})", sig.rsrq, sig.rsrq_index));
        } else {
            lines.push(String::from("Signal: -"));
        }

        lines.join("\n")
    }

    fn start_battery_load(&mut self) {
        if self.battery_loading {
            return;
        }

        self.battery_loading = true;
        self.battery_info_text = String::from("Loading battery info...");

        executor::run(async {
            let event = match jetbeep_core::bus::battery_get_info().await {
                Ok(info) => AppEvent::BatteryInfoLoaded {
                    ok: true,
                    text: App::format_battery_info(&info),
                },
                Err(err) => AppEvent::BatteryInfoLoaded {
                    ok: false,
                    text: format!("{}", err),
                },
            };

            post_app_event(event);
        });
    }

    fn format_battery_info(info: &jetbeep_core::bus::BatteryInfo) -> String {
        let status_text = match info.status {
            0 => "Unknown",
            1 => "Charging",
            2 => "Not charging",
            3 => "Full",
            _ => "Other",
        };

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("Percent: {}%", info.percent));
        lines.push(format!("Voltage: {} mV", info.voltage));
        lines.push(format!("Charge status: {}", status_text));
        lines.push(format!("Temperature: {:.1} C", info.temperature));
        lines.push(format!("BMS firmware: {}", info.firmware_version));

        if info.cell_voltage.is_empty() {
            lines.push(String::from("Cell voltages: -"));
        } else {
            let mut cells = String::new();
            for (idx, mv) in info.cell_voltage.iter().enumerate() {
                if idx > 0 {
                    cells.push_str(", ");
                }
                cells.push_str(format!("C{}={}mV", idx + 1, mv).as_str());
            }
            lines.push(format!("Cell voltages: {}", cells));
        }

        lines.join("\n")
    }

    fn start_open_all_locks(&mut self) {
        if self.lock_open_all_running {
            return;
        }

        let targets: Vec<LockTarget> = self
            .lock_targets
            .iter()
            .filter(|t| t.enabled)
            .copied()
            .collect();
        let skipped = self.lock_targets.len() - targets.len();

        self.lock_open_all_running = true;
        self.lock_open_all_opened = 0;
        self.lock_open_all_ok = 0;
        self.lock_open_all_err = 0;
        self.lock_open_all_total = targets.len() as u32;
        self.lock_open_all_lines.clear();

        if targets.is_empty() {
            self.push_lock_open_all_line(if skipped > 0 {
                format!("No enabled locks ({} disabled skipped)", skipped)
            } else {
                String::from("No locks found in settings")
            });
            self.lock_open_all_running = false;
            self.render();
            return;
        }

        self.push_lock_open_all_line(if skipped > 0 {
            format!("Open all started ({} locks, {} disabled skipped)", targets.len(), skipped)
        } else {
            format!("Open all started ({} locks)", targets.len())
        });
        self.render();

        executor::run(async move {
            let total = targets.len();
            for (index, target) in targets.into_iter().enumerate() {
                let result = jetbeep_core::bus::lock_open(target.board_id, target.lock_id).await;
                let (ok, message) = match result {
                    Ok(()) => (true, String::from("opened")),
                    Err(err) => (false, format!("{}", err)),
                };

                post_app_event(AppEvent::LockOpenAllStepResult {
                    board_id: target.board_id,
                    lock_id: target.lock_id,
                    ok,
                    message,
                    index,
                    total,
                });

                workq::delay(Duration::from_millis(120)).await;
            }

            post_app_event(AppEvent::LockOpenAllFinished);
        });
    }

    fn push_lock_open_all_line(&mut self, line: String) {
        self.lock_open_all_lines.push(line);
        while self.lock_open_all_lines.len() > MAX_LOCK_LINES {
            self.lock_open_all_lines.remove(0);
        }
    }

    fn handle_afk_tick(&mut self) {
        if self.sleep_requested {
            return;
        }

        // Keep the AFK timer refreshed while "open all locks" is running:
        // the whole operation can easily exceed the AFK timeout.
        if self.lock_open_all_running {
            self.mark_user_activity();
            return;
        }

        if self.afk_seconds_left() == 0 {
            self.request_sleep();
        }
    }

    fn afk_seconds_left(&self) -> u32 {
        let elapsed = Self::current_unix_seconds().saturating_sub(self.last_user_activity_secs);
        if elapsed >= AFK_TIMEOUT_SECS {
            0
        } else {
            (AFK_TIMEOUT_SECS - elapsed) as u32
        }
    }

    fn mark_user_activity(&mut self) {
        self.last_user_activity_secs = Self::current_unix_seconds();
    }

    fn request_sleep(&mut self) {
        if self.sleep_requested {
            return;
        }

        self.sleep_requested = true;
        executor::run(async {
            if let Err(err) = jetbeep_core::bus::sleep().await {
                log::warn!("service-menu sleep failed: {}", err);
            }
        });
    }

    fn current_unix_seconds() -> i64 {
        (jetbeep_core::unix_time() / 1_000_000) as i64
    }

    fn required_keypad_mask(&self) -> u32 {
        let mut mask = (1u32 << 12) - 1; // 0-9,*,#
        if self.keypad_has_letters {
            mask |= ((1u32 << 4) - 1) << 12; // A-D
        }
        mask
    }

    fn track_keypad_key(&mut self, key: KeypadKey) {
        self.keypad_last_pressed = Some(key);
        let bit = match key {
            KeypadKey::Digit(1) => Some(0),
            KeypadKey::Digit(2) => Some(1),
            KeypadKey::Digit(3) => Some(2),
            KeypadKey::Digit(4) => Some(3),
            KeypadKey::Digit(5) => Some(4),
            KeypadKey::Digit(6) => Some(5),
            KeypadKey::Digit(7) => Some(6),
            KeypadKey::Digit(8) => Some(7),
            KeypadKey::Digit(9) => Some(8),
            KeypadKey::Star => Some(9),
            KeypadKey::Digit(0) => Some(10),
            KeypadKey::Hash => Some(11),
            KeypadKey::A => Some(12),
            KeypadKey::B => Some(13),
            KeypadKey::C => Some(14),
            KeypadKey::D => Some(15),
            _ => None,
        };

        if let Some(bit) = bit {
            self.keypad_pressed_mask |= 1u32 << bit;
        }
    }

    fn keypad_missing_labels(&self) -> Vec<&'static str> {
        let required = [
            (0, "1"),
            (1, "2"),
            (2, "3"),
            (3, "4"),
            (4, "5"),
            (5, "6"),
            (6, "7"),
            (7, "8"),
            (8, "9"),
            (9, "*"),
            (10, "0"),
            (11, "#"),
            (12, "A"),
            (13, "B"),
            (14, "C"),
            (15, "D"),
        ];

        let mut out = Vec::new();
        for (bit, label) in required {
            if bit >= 12 && !self.keypad_has_letters {
                continue;
            }
            if (self.keypad_pressed_mask & (1u32 << bit)) == 0 {
                out.push(label);
            }
        }
        out
    }

    fn reset_keypad_test(&mut self) {
        self.keypad_pressed_mask = 0;
        self.keypad_last_pressed = None;
    }

    fn reset_submenu_state(&mut self, route: Route) {
        if route == Route::MainMenu {
            return;
        }

        self.action_count = 0;

        if route == Route::KeypadTest {
            self.reset_keypad_test();
        }

        if route == Route::LockTestsOpenAll {
            self.lock_open_all_running = false;
            self.lock_open_all_opened = 0;
            self.lock_open_all_ok = 0;
            self.lock_open_all_err = 0;
            self.lock_open_all_total = self.enabled_lock_count() as u32;
            self.lock_open_all_lines.clear();
        }

        if route == Route::LockTestsOpenSpecificBoard {
            self.lock_open_specific_board_input.clear();
            self.lock_open_specific_lock_input.clear();
            self.lock_open_specific_board_id = None;
            self.lock_open_specific_running = false;
            self.lock_open_specific_status = String::from("Enter board_id");
        }

        if route == Route::LockTestsGetStatuses {
            self.lock_statuses_board_input.clear();
            self.lock_statuses_running = false;
            self.lock_statuses_input_status = String::from("Enter board_id");
            self.lock_statuses_result_status = String::from("No result yet");
        }
    }
}
