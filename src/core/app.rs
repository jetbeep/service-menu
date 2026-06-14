use alloc::{format, string::String, vec, vec::Vec};

use jetbeep::bus::KeypadKey;
use jetbeep_lvgl_dsl::prelude::{Obj, Widget};

use crate::core::{event::AppEvent, nav::Stack, route::Route};

pub struct App {
    route_stack: Stack<Route>,
    root: Option<Obj>,
    action_count: u32,
    keypad_has_letters: bool,
    keypad_pressed_mask: u32,
    keypad_last_pressed: Option<KeypadKey>,
}

impl App {
    pub fn new() -> Self {
        let mut route_stack = Stack::new();
        route_stack.push(Route::MainMenu);
        Self {
            route_stack,
            root: None,
            action_count: 0,
            keypad_has_letters: false,
            keypad_pressed_mask: 0,
            keypad_last_pressed: None,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Open(route) => {
                self.reset_submenu_state(route);
                self.route_stack.push(route);
                self.render();
            }
            AppEvent::Back => {
                if self.route_stack.len() > 1 {
                    self.route_stack.pop();
                    self.render();
                }
            }
            AppEvent::Action => {
                if self.current_route() != Route::MainMenu {
                    self.action_count = self.action_count.saturating_add(1);
                    self.render();
                }
            }
            AppEvent::Tick => self.render(),
            AppEvent::KeypadLayoutLoaded(has_letters) => {
                self.keypad_has_letters = has_letters;
                self.render();
            }
            AppEvent::Keypad(key) => self.handle_keypad(key),
        }
    }

    pub fn render(&mut self) {
        if let Some(root) = self.root.take() {
            root.delete();
        }
        self.root = Some(crate::ui::render(self));
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

        match self.current_route() {
            Route::MainMenu => {
                if let KeypadKey::Digit(d) = key {
                    if let Some(route) = Route::from_menu_digit(d) {
                        self.route_stack.push(route);
                        self.render();
                    }
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
                    self.action_count = self.action_count.saturating_add(1);
                    self.render();
                }
                _ => {}
            },
        }
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

        // Placeholder submenus currently share this action counter.
        // Reset on every open so each menu starts from a clean state.
        self.action_count = 0;

        if route == Route::KeypadTest {
            self.reset_keypad_test();
        }
    }
}
