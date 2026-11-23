use alloc::{format, rc::Rc, string::String, vec, vec::Vec};
use ::core::{cell::Cell, time::Duration};

use jetbeep_core::bus::{JkvValue, KeypadKey};
use jetbeep_core::proto::bus::{LockStatus, NetworkRegistrationStatus};
use jetbeep_core::{executor, workq};

use crate::core::{
    app_dispatcher::post_app_event,
    event::{AppEvent, LockTarget},
    nav::Stack,
    route::Route,
};

const MAX_LOCK_LINES: usize = 256;
const AFK_TIMEOUT_SECS: u32 = 60;

fn lock_status_for_id(statuses: &[LockStatus], lock_id: u32) -> LockStatus {
    lock_id
        .checked_sub(1)
        .and_then(|index| statuses.get(index as usize))
        .copied()
        .unwrap_or(LockStatus::Disabled)
}

pub struct App {
    route_stack: Stack<Route>,
    ui: Option<crate::ui::Ui>,
    action_count: u32,
    keypad_has_letters: bool,
    keypad_pressed_mask: u32,
    keypad_last_pressed: Option<KeypadKey>,
    lock_targets: Vec<LockTarget>,
    lock_open_all_running: bool,
    /// Set to `true` to make the running open-all task stop before its next lock.
    lock_open_all_cancel: Option<Rc<Cell<bool>>>,
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
    /// Clock ticks since the last user activity. Counted per tick rather than
    /// derived from `unix_time`, which jumps whenever the mainboard sets the time.
    idle_secs: u32,
    sleep_requested: bool,
}

impl App {
    pub fn new() -> Self {
        let mut route_stack = Stack::new();
        route_stack.push(Route::MainMenu);
        Self {
            route_stack,
            ui: None,
            action_count: 0,
            keypad_has_letters: false,
            keypad_pressed_mask: 0,
            keypad_last_pressed: None,
            lock_targets: Vec::new(),
            lock_open_all_running: false,
            lock_open_all_cancel: None,
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
            idle_secs: 0,
            sleep_requested: false,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Open(route) => {
                self.mark_user_activity();
                self.open_route(route);
            }
            AppEvent::Back => {
                self.mark_user_activity();
                if self.lock_open_all_running && self.current_route() == Route::LockTestsOpenAll {
                    self.stop_open_all_locks();
                } else if self.route_stack.len() > 1 {
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
                self.refresh_clock();
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
                // A step queued just before Stop was handled belongs to a run
                // that is already over.
                if !self.lock_open_all_running {
                    return;
                }
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
                self.refresh_open_all();
            }
            AppEvent::LockOpenAllFinished => {
                if !self.lock_open_all_running {
                    return;
                }
                self.mark_user_activity();
                self.lock_open_all_running = false;
                self.lock_open_all_cancel = None;
                self.push_lock_open_all_line(format!(
                    "Done: {} OK, {} ERROR",
                    self.lock_open_all_ok,
                    self.lock_open_all_err
                ));
                self.refresh_open_all();
                self.refresh_clock();
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
        if let Some(ui) = self.ui.take() {
            ui.delete();
        }
        self.ui = Some(crate::ui::render(self));
    }

    fn refresh_clock(&self) {
        if let Some(ui) = self.ui.as_ref() {
            ui.update_clock(self);
        }
    }

    /// Open-all results arrive every few hundred ms; updating the page in place
    /// keeps Back/Run tappable and the results scroll position intact.
    fn refresh_open_all(&self) {
        if let Some(ui) = self.ui.as_ref() {
            ui.update_open_all(self);
        }
    }

    /// Delete the whole UI tree (soft-kill path).
    pub fn teardown(&mut self) {
        if let Some(ui) = self.ui.take() {
            ui.delete();
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
        if self.afk_paused() {
            return String::from("AFK: paused");
        }
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
                            self.open_route(route);
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
                    if self.lock_open_all_running {
                        self.stop_open_all_locks();
                    } else if self.route_stack.len() > 1 {
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

    /// Single entry point for opening a screen, shared by touch and keypad so
    /// both start the screen from the same clean state.
    fn open_route(&mut self, route: Route) {
        self.reset_submenu_state(route);
        self.route_stack.push(route);

        match route {
            Route::BarcodeScannerTest => self.start_barcode_scanner(),
            Route::ConnectivityInfo => self.start_connectivity_load(),
            Route::BatteryInfo => self.start_battery_load(),
            _ => {}
        }

        self.render();
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
            let state = lock_status_for_id(&statuses, lock_id);

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

        let cancel = Rc::new(Cell::new(false));
        self.lock_open_all_cancel = Some(cancel.clone());
        executor::run(async move {
            let total = targets.len();
            for (index, target) in targets.into_iter().enumerate() {
                if cancel.get() {
                    return;
                }
                let result = jetbeep_core::bus::lock_open(target.board_id, target.lock_id).await;
                // The lock was already commanded, but the run is over: report nothing.
                if cancel.get() {
                    return;
                }
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

            if !cancel.get() {
                post_app_event(AppEvent::LockOpenAllFinished);
            }
        });
    }

    /// Stop the open-all run before its next lock. A lock whose command is
    /// already in flight on the bus still opens; its result is not shown.
    fn stop_open_all_locks(&mut self) {
        if let Some(cancel) = self.lock_open_all_cancel.take() {
            cancel.set(true);
        }
        self.lock_open_all_running = false;
        self.push_lock_open_all_line(format!(
            "Stopped: {} OK, {} ERROR",
            self.lock_open_all_ok,
            self.lock_open_all_err
        ));
        self.refresh_open_all();
        self.refresh_clock();
    }

    fn push_lock_open_all_line(&mut self, line: String) {
        self.lock_open_all_lines.push(line);
        while self.lock_open_all_lines.len() > MAX_LOCK_LINES {
            self.lock_open_all_lines.remove(0);
        }
    }

    fn handle_afk_tick(&mut self) {
        if self.sleep_requested || self.afk_paused() {
            return;
        }

        self.idle_secs = self.idle_secs.saturating_add(1);
        if self.afk_seconds_left() == 0 {
            self.request_sleep();
        }
    }

    /// Opening every lock can take longer than the AFK timeout, so the
    /// countdown stops for the whole run.
    fn afk_paused(&self) -> bool {
        self.lock_open_all_running
    }

    fn afk_seconds_left(&self) -> u32 {
        AFK_TIMEOUT_SECS.saturating_sub(self.idle_secs)
    }

    fn mark_user_activity(&mut self) {
        self.idle_secs = 0;
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use ::core::cell::Cell;
    use lvgl_dsl::c_bindings::{spy_drain, LvCall};
    use lvgl_dsl::test_support::SpyFixture;

    #[test]
    fn clock_tick_updates_labels_without_rebuilding_screen() {
        let _spy = SpyFixture::new();
        let mut app = App::new();
        app.render();
        spy_drain();

        app.handle_event(AppEvent::Tick);

        let calls = spy_drain();
        let count = |pred: fn(&LvCall) -> bool| calls.iter().filter(|c| pred(c)).count();
        // Deleting the widget under a finger makes LVGL drop the click
        // (lv_indev_wait_release), so a tick must never rebuild the tree.
        assert_eq!(count(|c| matches!(c, LvCall::ObjDelete { .. })), 0, "tick deleted widgets");
        assert_eq!(
            count(|c| matches!(
                c,
                LvCall::ObjCreate { .. } | LvCall::LabelCreate { .. } | LvCall::ButtonCreate { .. }
            )),
            0,
            "tick created widgets"
        );
        assert_eq!(
            count(|c| matches!(c, LvCall::LabelSetText { .. })),
            3,
            "tick must refresh exactly the date, AFK and time labels"
        );
    }

    #[test]
    fn keypad_test_opened_from_keypad_starts_from_scratch() {
        let _spy = SpyFixture::new();
        let mut app = App::new();
        app.render();

        app.handle_event(AppEvent::Keypad(KeypadKey::Digit(1)));
        app.handle_event(AppEvent::Keypad(KeypadKey::Digit(5)));
        app.handle_event(AppEvent::Keypad(KeypadKey::Hash));
        assert_eq!(app.current_route(), Route::MainMenu);

        app.handle_event(AppEvent::Keypad(KeypadKey::Digit(1)));

        assert_eq!(app.current_route(), Route::KeypadTest);
        assert_eq!(app.action_text(), "Missing: 1 2 3 4 5 6 7 8 9 * 0 #");
    }

    fn open_all_step(index: usize) -> AppEvent {
        AppEvent::LockOpenAllStepResult {
            board_id: 1,
            lock_id: index as u32 + 1,
            ok: true,
            message: String::from("opened"),
            index,
            total: 27,
        }
    }

    #[test]
    fn afk_countdown_follows_ticks_not_wall_clock() {
        let _spy = SpyFixture::new();
        let mut app = App::new();
        app.render();

        for _ in 0..5 {
            app.handle_event(AppEvent::Tick);
        }

        assert_eq!(app.afk_countdown_text(), "AFK: 55s");
    }

    #[test]
    fn user_activity_restarts_afk_countdown() {
        let _spy = SpyFixture::new();
        let mut app = App::new();
        app.render();
        for _ in 0..30 {
            app.handle_event(AppEvent::Tick);
        }

        app.handle_event(AppEvent::Open(Route::LockTests));

        assert_eq!(app.afk_countdown_text(), "AFK: 60s");
    }

    #[test]
    fn afk_is_paused_while_open_all_runs() {
        let _spy = SpyFixture::new();
        let mut app = App::new();
        app.render();
        app.lock_open_all_running = true;

        for _ in 0..(AFK_TIMEOUT_SECS + 10) {
            app.handle_event(AppEvent::Tick);
        }

        assert!(!app.sleep_requested);
        assert_eq!(app.afk_countdown_text(), "AFK: paused");
    }

    #[test]
    fn open_all_step_updates_page_without_rebuilding_screen() {
        let _spy = SpyFixture::new();
        let mut app = App::new();
        app.render();
        app.handle_event(AppEvent::Open(Route::LockTests));
        app.handle_event(AppEvent::Open(Route::LockTestsOpenAll));
        app.lock_open_all_running = true;
        spy_drain();

        app.handle_event(open_all_step(0));
        app.handle_event(AppEvent::LockOpenAllFinished);

        let calls = spy_drain();
        let count = |pred: fn(&LvCall) -> bool| calls.iter().filter(|c| pred(c)).count();
        assert_eq!(count(|c| matches!(c, LvCall::ObjDelete { .. })), 0, "step deleted widgets");
        assert_eq!(
            count(|c| matches!(
                c,
                LvCall::ObjCreate { .. } | LvCall::LabelCreate { .. } | LvCall::ButtonCreate { .. }
            )),
            0,
            "step created widgets"
        );
        assert!(
            count(|c| matches!(c, LvCall::LabelSetText { .. })) > 0,
            "step must refresh the page labels"
        );
        assert!(!app.lock_open_all_running());
        assert_eq!(app.lock_open_all_summary_text(), "Last run: 1/27 | 1 OK | 0 ERROR");
    }

    /// Opens the open-all page with a run in progress, without spawning the
    /// real lock task. Returns the run's cancel flag.
    fn app_with_open_all_running() -> (App, Rc<Cell<bool>>) {
        let mut app = App::new();
        app.render();
        app.handle_event(AppEvent::Open(Route::LockTests));
        app.handle_event(AppEvent::Open(Route::LockTestsOpenAll));
        let cancel = Rc::new(Cell::new(false));
        app.lock_open_all_running = true;
        app.lock_open_all_cancel = Some(cancel.clone());
        app.render();
        (app, cancel)
    }

    fn label_texts(calls: &[LvCall]) -> Vec<String> {
        calls
            .iter()
            .filter_map(|c| match c {
                LvCall::LabelSetText { text_bytes, .. } => {
                    Some(String::from_utf8_lossy(text_bytes).trim_end_matches('\0').into())
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn back_button_reads_stop_while_open_all_runs() {
        let _spy = SpyFixture::new();
        let (mut app, _cancel) = app_with_open_all_running();
        spy_drain();

        app.render();
        assert!(label_texts(&spy_drain()).iter().any(|t| t == "Stop (*)"));

        app.handle_event(AppEvent::LockOpenAllFinished);
        assert!(label_texts(&spy_drain()).iter().any(|t| t == "Back (*)"));
    }

    #[test]
    fn first_back_stops_open_all_and_second_leaves_page() {
        let _spy = SpyFixture::new();
        let (mut app, cancel) = app_with_open_all_running();
        app.handle_event(open_all_step(0));

        app.handle_event(AppEvent::Back);

        assert!(cancel.get(), "the lock task must be told to stop");
        assert!(!app.lock_open_all_running());
        assert_eq!(app.current_route(), Route::LockTestsOpenAll);
        assert_eq!(app.lock_open_all_summary_text(), "Last run: 1/27 | 1 OK | 0 ERROR");

        app.handle_event(AppEvent::Back);

        assert_eq!(app.current_route(), Route::LockTests);
    }

    #[test]
    fn keypad_star_stops_open_all_like_back() {
        let _spy = SpyFixture::new();
        let (mut app, cancel) = app_with_open_all_running();

        app.handle_event(AppEvent::Keypad(KeypadKey::Star));

        assert!(cancel.get());
        assert_eq!(app.current_route(), Route::LockTestsOpenAll);
    }

    #[test]
    fn open_all_result_arriving_after_stop_is_ignored() {
        let _spy = SpyFixture::new();
        let (mut app, _cancel) = app_with_open_all_running();
        app.handle_event(AppEvent::Back);

        app.handle_event(open_all_step(0));
        app.handle_event(AppEvent::LockOpenAllFinished);

        assert_eq!(app.lock_open_all_ok, 0);
        assert!(!app.lock_open_all_results_text().contains("Done"));
    }

    #[test]
    fn status_vector_uses_one_based_lock_ids() {
        let statuses = [LockStatus::Opened, LockStatus::Closed, LockStatus::Opened, LockStatus::Closed];

        assert_eq!(lock_status_for_id(&statuses, 1), LockStatus::Opened);
        assert_eq!(lock_status_for_id(&statuses, 4), LockStatus::Closed);
        assert_eq!(lock_status_for_id(&statuses, 0), LockStatus::Disabled);
    }
}
