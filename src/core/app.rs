use alloc::string::String;

use jetbeep::bus::KeypadKey;
use jetbeep_lvgl_dsl::prelude::{Obj, Widget};

use crate::core::{event::AppEvent, nav::Stack, route::Route};

pub struct App {
    route_stack: Stack<Route>,
    root: Option<Obj>,
    action_count: u32,
}

impl App {
    pub fn new() -> Self {
        let mut route_stack = Stack::new();
        route_stack.push(Route::MainMenu);
        Self {
            route_stack,
            root: None,
            action_count: 0,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Open(route) => {
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

    fn handle_keypad(&mut self, key: KeypadKey) {
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
}
