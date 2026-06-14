use alloc::{format, string::String};

use jetbeep::bus::KeypadKey;
use jetbeep_lvgl_dsl::prelude::*;

use crate::core::{
    app::App,
    app_dispatcher::post_app_event,
    event::AppEvent,
    route::Route,
};

const BG: u32 = 0xF4F6F8;
const CARD: u32 = 0xFFFFFF;
const PRIMARY: u32 = 0x0F4C81;
const TEXT: u32 = 0x1F2937;
const MUTED: u32 = 0x5B6472;
const ROOT_PAD: i32 = 16;
const ROOT_GAP: i32 = 8;
const MENU_BUTTON_H: i32 = 60;
const ACTION_BUTTON_H: i32 = 60;

pub fn render(app: &App) -> Obj {
    let screen = Screen::active();
    let root = Obj::new(&screen);
    root.width(Size::Pct(100))
        .height(Size::Pct(100))
        .bg_color(Color::hex(BG))
        .pad_all(ROOT_PAD)
        .border_width(0)
        .flex_col()
        .flex_align(FlexAlign::Start, FlexAlign::Start, FlexAlign::Start)
        .gap(ROOT_GAP)
        .remove_flag(LvObjFlag::SCROLLABLE);

    render_clock_overlay(&root);

    match app.current_route() {
        Route::MainMenu => render_main_menu(&root),
        Route::KeypadTest => render_keypad_test(&root, app),
        route => render_test_screen(&root, route, &app.action_text()),
    }

    root
}

fn render_clock_overlay(root: &Obj) {
    let date = Label::new(root);
    date
        .add_flag(LvObjFlag::FLOATING | LvObjFlag::IGNORE_LAYOUT)
        .align(LvAlign::TopRight, 0, 0)
        .text(&format_utc_date())
        .text_color(Color::hex(MUTED));

    let time = Label::new(root);
    time
        .add_flag(LvObjFlag::FLOATING | LvObjFlag::IGNORE_LAYOUT)
        .align(LvAlign::TopRight, 0, 18)
        .text(&format_utc_time())
        .text_color(Color::hex(MUTED));
}

fn format_utc_date() -> String {
    let micros = jetbeep::unix_time();
    let total_seconds = (micros / 1_000_000) as i64;
    let days_since_epoch = total_seconds.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days_since_epoch);

    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn format_utc_time() -> String {
    let micros = jetbeep::unix_time();
    let total_seconds = (micros / 1_000_000) as i64;
    let day_seconds = total_seconds.rem_euclid(86_400);

    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;

    format!(
        "{:02}:{:02}:{:02} UTC",
        hour, minute, second
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

fn render_main_menu(root: &Obj) {
    let title = Label::new(root);
    title.text("Service Menu").text_color(Color::hex(TEXT));

    let subtitle = Label::new(root);
    subtitle
        .text("Select by touch, or press keypad 1-6")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    add_menu_button(root, "1. Keypad test", on_menu_1_click);
    add_menu_button(root, "2. Lock tests", on_menu_2_click);
    add_menu_button(root, "3. Barcode scanner test", on_menu_3_click);
    add_menu_button(root, "4. Connectivity info", on_menu_4_click);
    add_menu_button(root, "5. Battery info", on_menu_5_click);
    add_menu_button(root, "6. Screen color test", on_menu_6_click);
}

fn add_menu_button(parent: &Obj, text: &str, on_click: fn(Event)) {
    let btn = Button::new(parent);
    btn.width(Size::Pct(100))
        .height(Size::Px(MENU_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .pad_left(14)
        .pad_right(14)
        .on_click(on_click)
        .text(text)
        .text_color(Color::hex(TEXT));
}

fn render_test_screen(root: &Obj, route: Route, action_text: &str) {
    let title = Label::new(root);
    title.text(route.title()).text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text(route.description()).text_color(Color::hex(MUTED));

    if !action_text.is_empty() {
        let status = Label::new(root);
        status
            .text(action_text)
            .text_color(Color::hex(PRIMARY))
            .long_mode(LvLabelLongMode::Wrap);
    }

    let spacer = Obj::new(root);
    spacer
        .width(Size::Pct(100))
        .height(Size::Px(8))
        .bg_opa(0)
        .border_width(0);

    let actions = Obj::new(root);
    actions
        .width(Size::Pct(100))
        .height(Size::Px(ACTION_BUTTON_H + 8))
        .bg_opa(0)
        .border_width(0)
        .flex_row()
        .flex_align(FlexAlign::SpaceBetween, FlexAlign::Center, FlexAlign::Center)
        .gap(12)
        .remove_flag(LvObjFlag::SCROLLABLE);

    let back = Button::new(&actions);
    back.width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_back_click)
        .text("Back (*)")
        .text_color(Color::hex(TEXT));

    let action = Button::new(&actions);
    action
        .width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_action_click)
        .text("Action (#)")
        .text_color(Color::white());
}

fn render_keypad_test(root: &Obj, app: &App) {
    let title = Label::new(root);
    title.text(Route::KeypadTest.title()).text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text(Route::KeypadTest.description())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let layout = Label::new(root);
    layout
        .text(&format!("Detected layout: {}", app.keypad_layout_name()))
        .text_color(Color::hex(MUTED));

    let required = Label::new(root);
    required
        .text(&app.keypad_required_keys_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(TEXT));

    let status = Label::new(root);
    status
        .text(&app.action_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(PRIMARY));

    let last = Label::new(root);
    last
        .text(&app.keypad_last_pressed_text())
        .text_color(Color::hex(MUTED));

    let spacer = Obj::new(root);
    spacer
        .width(Size::Pct(100))
        .height(Size::Px(8))
        .bg_opa(0)
        .border_width(0);

    let actions = Obj::new(root);
    actions
        .width(Size::Pct(100))
        .height(Size::Px(ACTION_BUTTON_H + 8))
        .bg_opa(0)
        .border_width(0)
        .flex_row()
        .flex_align(FlexAlign::SpaceBetween, FlexAlign::Center, FlexAlign::Center)
        .gap(12)
        .remove_flag(LvObjFlag::SCROLLABLE);

    let back = Button::new(&actions);
    back.width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_back_click)
        .text("Back")
        .text_color(Color::hex(TEXT));

    let done = Button::new(&actions);
    done.width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_keypad_done_click)
        .text("Done (#)")
        .text_color(Color::white());
}

fn on_menu_1_click(_: Event) {
    post_app_event(AppEvent::Open(Route::KeypadTest));
}
fn on_menu_2_click(_: Event) {
    post_app_event(AppEvent::Open(Route::LockTests));
}
fn on_menu_3_click(_: Event) {
    post_app_event(AppEvent::Open(Route::BarcodeScannerTest));
}
fn on_menu_4_click(_: Event) {
    post_app_event(AppEvent::Open(Route::ConnectivityInfo));
}
fn on_menu_5_click(_: Event) {
    post_app_event(AppEvent::Open(Route::BatteryInfo));
}
fn on_menu_6_click(_: Event) {
    post_app_event(AppEvent::Open(Route::ScreenColorTest));
}

fn on_back_click(_: Event) {
    post_app_event(AppEvent::Back);
}

fn on_action_click(_: Event) {
    post_app_event(AppEvent::Action);
}

fn on_keypad_done_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Hash));
}
