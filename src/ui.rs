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
const OPEN_SPEC_ACTION_BUTTON_H: i32 = 52;
const NUMPAD_KEY_H: i32 = OPEN_SPEC_ACTION_BUTTON_H;
const NUMPAD_GAP: i32 = 6;
const NUMPAD_ROWS: i32 = 4;

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
        Route::LockTests => render_lock_tests_menu(&root),
        Route::LockTestsOpenAll => render_open_all_page(&root, app),
        Route::LockTestsOpenSpecificBoard => render_open_specific_board_page(&root, app),
        Route::LockTestsOpenSpecificLock => render_open_specific_lock_page(&root, app),
        Route::LockTestsGetStatuses => render_get_statuses_page(&root, app),
        Route::LockTestsGetStatusesResult => render_get_statuses_result_page(&root, app),
        Route::BarcodeScannerTest => render_barcode_scanner_page(&root, app),
        Route::ConnectivityInfo => render_connectivity_info_page(&root, app),
        Route::BatteryInfo => render_battery_info_page(&root, app),
        Route::ScreenColorTest => render_screen_color_test_page(&root),
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

    fn add_bottom_spacer(parent: &Obj) {
        let spacer = Obj::new(parent);
        spacer
        .width(Size::Pct(100))
        .height(Size::Px(1))
        .flex_grow(1)
        .bg_opa(0)
        .border_width(0)
        .remove_flag(LvObjFlag::SCROLLABLE);
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

    add_bottom_spacer(root);

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

fn render_lock_tests_menu(root: &Obj) {
    let title = Label::new(root);
    title.text(Route::LockTests.title()).text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc
        .text(Route::LockTests.description())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    add_menu_button(root, "1. Open all locks", on_lock_menu_1_click);
    add_menu_button(root, "2. Open specific lock", on_lock_menu_2_click);
    add_menu_button(root, "3. Get lock statuses of board", on_lock_menu_3_click);

    add_bottom_spacer(root);

    let back = Button::new(root);
    back.width(Size::Pct(100))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_back_click)
        .text("Back (*)")
        .text_color(Color::hex(TEXT));
}

fn render_open_all_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title.text(Route::LockTestsOpenAll.title()).text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Open every lock from device settings and show result for each lock.")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let configured = Label::new(root);
    configured
        .text(&format!("Configured locks: {}", app.lock_open_all_configured_count()))
        .text_color(Color::hex(MUTED));

    let summary = Label::new(root);
    summary
        .text(&app.lock_open_all_summary_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(PRIMARY));

    let results = Label::new(root);
    results
        .text(&app.lock_open_all_results_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(TEXT));

    add_bottom_spacer(root);

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

    let run = Button::new(&actions);
    run.width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_lock_open_all_run_click)
        .text(if app.lock_open_all_running() {
            "Running..."
        } else {
            "Run (#)"
        })
        .text_color(Color::white());
}

fn render_open_specific_board_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title
        .text(Route::LockTestsOpenSpecificBoard.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Step 1/2: enter board_id using keypad or touch buttons.")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let available = Label::new(root);
    available
        .text(&app.lock_open_specific_available_boards_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let input = Label::new(root);
    input
        .text(&format!("Board ID: {}", app.lock_open_specific_board_input_text()))
        .text_color(Color::hex(TEXT));

    let status = Label::new(root);
    status
        .text(&app.lock_open_specific_status_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(PRIMARY));

    render_numeric_touch_keypad(root);

    add_bottom_spacer(root);

    let actions = Obj::new(root);
    actions
        .width(Size::Pct(100))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_opa(0)
        .border_width(0)
        .flex_row()
        .flex_align(FlexAlign::SpaceBetween, FlexAlign::Center, FlexAlign::Center)
        .gap(12)
        .remove_flag(LvObjFlag::SCROLLABLE);

    let back = Button::new(&actions);
    back.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_open_specific_back_or_del_click)
        .text(if app.lock_open_specific_board_has_input() {
            "Del (*)"
        } else {
            "Back (*)"
        })
        .text_color(Color::hex(TEXT));

    let next = Button::new(&actions);
    next.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_open_specific_submit_click)
        .text("Next (#)")
        .text_color(Color::white());
}

fn render_open_specific_lock_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title
        .text(Route::LockTestsOpenSpecificLock.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Step 2/2: enter lock_id, then press # to open lock.")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let available = Label::new(root);
    available
        .text(&app.lock_open_specific_available_locks_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let input = Label::new(root);
    input
        .text(&format!("Lock ID: {}", app.lock_open_specific_lock_input_text()))
        .text_color(Color::hex(TEXT));

    let status = Label::new(root);
    status
        .text(&app.lock_open_specific_status_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(PRIMARY));

    render_numeric_touch_keypad(root);

    add_bottom_spacer(root);

    let actions = Obj::new(root);
    actions
        .width(Size::Pct(100))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_opa(0)
        .border_width(0)
        .flex_row()
        .flex_align(FlexAlign::SpaceBetween, FlexAlign::Center, FlexAlign::Center)
        .gap(12)
        .remove_flag(LvObjFlag::SCROLLABLE);

    let back = Button::new(&actions);
    back.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_open_specific_back_or_del_click)
        .text(if app.lock_open_specific_lock_has_input() {
            "Del (*)"
        } else {
            "Back (*)"
        })
        .text_color(Color::hex(TEXT));

    let run = Button::new(&actions);
    run.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_open_specific_submit_click)
        .text(if app.lock_open_specific_running() {
            "Opening..."
        } else if app.lock_open_specific_lock_has_input() {
            "Open (#)"
        } else {
            "Exit (#)"
        })
        .text_color(Color::white());
}

fn render_get_statuses_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title.text(Route::LockTestsGetStatuses.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Enter board_id, then press # to request lock statuses.")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let available = Label::new(root);
    available
        .text(&app.lock_open_specific_available_boards_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let input = Label::new(root);
    input
        .text(&format!("Board ID: {}", app.lock_statuses_board_input_text()))
        .text_color(Color::hex(TEXT));

    let status = Label::new(root);
    status
        .text(&app.lock_statuses_input_status_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(PRIMARY));

    render_numeric_touch_keypad(root);

    add_bottom_spacer(root);

    let actions = Obj::new(root);
    actions
        .width(Size::Pct(100))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_opa(0)
        .border_width(0)
        .flex_row()
        .flex_align(FlexAlign::SpaceBetween, FlexAlign::Center, FlexAlign::Center)
        .gap(12)
        .remove_flag(LvObjFlag::SCROLLABLE);

    let back = Button::new(&actions);
    back.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_get_statuses_back_or_del_click)
        .text(if app.lock_statuses_board_has_input() {
            "Del (*)"
        } else {
            "Back (*)"
        })
        .text_color(Color::hex(TEXT));

    let run = Button::new(&actions);
    run.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_get_statuses_submit_click)
        .text(if app.lock_statuses_running() {
            "Running..."
        } else {
            "Run (#)"
        })
        .text_color(Color::white());
}

fn render_get_statuses_result_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title
        .text(Route::LockTestsGetStatusesResult.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Review result, go back to edit board_id, or exit to main menu.")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let result = Label::new(root);
    result
        .text(&app.lock_statuses_result_status_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(PRIMARY));

    add_bottom_spacer(root);

    let actions = Obj::new(root);
    actions
        .width(Size::Pct(100))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_opa(0)
        .border_width(0)
        .flex_row()
        .flex_align(FlexAlign::SpaceBetween, FlexAlign::Center, FlexAlign::Center)
        .gap(12)
        .remove_flag(LvObjFlag::SCROLLABLE);

    let back = Button::new(&actions);
    back.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_get_statuses_result_back_click)
        .text("Back (*)")
        .text_color(Color::hex(TEXT));

    let exit = Button::new(&actions);
    exit.width(Size::Pct(48))
        .height(Size::Px(OPEN_SPEC_ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_get_statuses_result_exit_click)
        .text("Exit (#)")
        .text_color(Color::white());
}

fn render_barcode_scanner_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title
        .text(Route::BarcodeScannerTest.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Scanner starts on enter. After each scan it restarts automatically.")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let status = Label::new(root);
    status
        .text(&app.barcode_status_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(PRIMARY));

    let last = Label::new(root);
    last
        .text(&format!("Last scanned: {}", app.barcode_last_text()))
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(TEXT));

    add_bottom_spacer(root);

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

    let restart = Button::new(&actions);
    restart.width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_barcode_restart_click)
        .text("Restart (#)")
        .text_color(Color::white());
}

fn render_connectivity_info_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title
        .text(Route::ConnectivityInfo.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Modem information from modem_get_info().")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let status = Label::new(root);
    status
        .text(if app.connectivity_loading() {
            "Loading modem info..."
        } else {
            "Modem info loaded"
        })
        .text_color(Color::hex(PRIMARY));

    let server = Label::new(root);
    server
        .text(&format!("Connected to server: {}", app.connectivity_server_state()))
        .text_color(Color::hex(PRIMARY));

    let info = Label::new(root);
    info
        .text(&app.connectivity_info_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(TEXT));

    add_bottom_spacer(root);

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

    let refresh = Button::new(&actions);
    refresh.width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_action_click)
        .text("Refresh (#)")
        .text_color(Color::white());
}

fn render_battery_info_page(root: &Obj, app: &App) {
    let title = Label::new(root);
    title
        .text(Route::BatteryInfo.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Battery information from battery_get_info().")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let status = Label::new(root);
    status
        .text(if app.battery_loading() {
            "Loading battery info..."
        } else {
            "Battery info loaded"
        })
        .text_color(Color::hex(PRIMARY));

    let info = Label::new(root);
    info
        .text(&app.battery_info_text())
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(TEXT));

    add_bottom_spacer(root);

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

    let refresh = Button::new(&actions);
    refresh.width(Size::Pct(48))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(PRIMARY))
        .border_width(0)
        .radius(10)
        .on_click(on_action_click)
        .text("Refresh (#)")
        .text_color(Color::white());
}

fn render_screen_color_test_page(root: &Obj) {
    let title = Label::new(root);
    title
        .text(Route::ScreenColorTest.title())
        .text_color(Color::hex(TEXT));

    let desc = Label::new(root);
    desc.text("Verify stripe colors and gradients on the display.")
        .long_mode(LvLabelLongMode::Wrap)
        .text_color(Color::hex(MUTED));

    let stripes = Obj::new(root);
    stripes
        .width(Size::Pct(100))
        .flex_grow(1)
        .bg_opa(0)
        .border_width(0)
        .pad_all(0)
        .flex_col()
        .gap(0)
        .remove_flag(LvObjFlag::SCROLLABLE);

    add_color_stripe(&stripes, 0xFFFFFF);
    add_color_stripe(&stripes, 0x000000);
    add_color_stripe(&stripes, 0xFF0000);
    add_color_stripe(&stripes, 0x00FF00);
    add_color_stripe(&stripes, 0x0000FF);
    add_color_stripe(&stripes, 0x00FFFF);
    add_color_stripe(&stripes, 0xFF00FF);
    add_color_stripe(&stripes, 0xFFFF00);

    let actions = Obj::new(root);
    actions
        .width(Size::Pct(100))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_opa(0)
        .border_width(0)
        .flex_row()
        .flex_align(FlexAlign::Center, FlexAlign::Center, FlexAlign::Center)
        .remove_flag(LvObjFlag::SCROLLABLE);

    let back = Button::new(&actions);
    back.width(Size::Pct(100))
        .height(Size::Px(ACTION_BUTTON_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_back_click)
        .text("Back (*)")
        .text_color(Color::hex(TEXT));
}

fn add_color_stripe(parent: &Obj, hex: u32) {
    let stripe = Obj::new(parent);
    stripe
        .width(Size::Pct(100))
        .flex_grow(1)
        .bg_color(Color::hex(hex))
        .border_width(0)
        .radius(0)
        .pad_all(0)
        .remove_flag(LvObjFlag::SCROLLABLE);
}

fn render_numeric_touch_keypad(root: &Obj) {
    let keypad = Obj::new(root);
    keypad
        .width(Size::Pct(100))
        .height(Size::Px((NUMPAD_KEY_H * NUMPAD_ROWS) + (NUMPAD_GAP * (NUMPAD_ROWS - 1))))
        .bg_opa(0)
        .border_width(0)
        .flex_row_wrap()
        .flex_align(FlexAlign::Center, FlexAlign::Center, FlexAlign::Center)
        .gap(NUMPAD_GAP)
        .remove_flag(LvObjFlag::SCROLLABLE);

    add_numeric_touch_key(&keypad, "1", on_touch_key_1_click);
    add_numeric_touch_key(&keypad, "2", on_touch_key_2_click);
    add_numeric_touch_key(&keypad, "3", on_touch_key_3_click);
    add_numeric_touch_key(&keypad, "4", on_touch_key_4_click);
    add_numeric_touch_key(&keypad, "5", on_touch_key_5_click);
    add_numeric_touch_key(&keypad, "6", on_touch_key_6_click);
    add_numeric_touch_key(&keypad, "7", on_touch_key_7_click);
    add_numeric_touch_key(&keypad, "8", on_touch_key_8_click);
    add_numeric_touch_key(&keypad, "9", on_touch_key_9_click);
    add_numeric_touch_key(&keypad, "0", on_touch_key_0_click);
}

fn add_numeric_touch_key(parent: &Obj, text: &str, on_click: fn(Event)) {
    let btn = Button::new(parent);
    btn.width(Size::Pct(31))
        .height(Size::Px(NUMPAD_KEY_H))
        .bg_color(Color::hex(CARD))
        .border_width(2)
        .border_color(Color::hex(0xD5DCE4))
        .radius(10)
        .on_click(on_click)
        .text(text)
        .text_color(Color::hex(TEXT));
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

fn on_barcode_restart_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Hash));
}

fn on_lock_menu_1_click(_: Event) {
    post_app_event(AppEvent::Open(Route::LockTestsOpenAll));
}

fn on_lock_menu_2_click(_: Event) {
    post_app_event(AppEvent::Open(Route::LockTestsOpenSpecificBoard));
}

fn on_lock_menu_3_click(_: Event) {
    post_app_event(AppEvent::Open(Route::LockTestsGetStatuses));
}

fn on_lock_open_all_run_click(_: Event) {
    post_app_event(AppEvent::LockOpenAll);
}

fn on_open_specific_submit_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Hash));
}

fn on_open_specific_back_or_del_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Star));
}

fn on_touch_key_1_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(1)));
}

fn on_touch_key_2_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(2)));
}

fn on_touch_key_3_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(3)));
}

fn on_touch_key_4_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(4)));
}

fn on_touch_key_5_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(5)));
}

fn on_touch_key_6_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(6)));
}

fn on_touch_key_7_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(7)));
}

fn on_touch_key_8_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(8)));
}

fn on_touch_key_9_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(9)));
}

fn on_touch_key_0_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Digit(0)));
}

fn on_get_statuses_submit_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Hash));
}

fn on_get_statuses_back_or_del_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Star));
}

fn on_get_statuses_result_back_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Star));
}

fn on_get_statuses_result_exit_click(_: Event) {
    post_app_event(AppEvent::Keypad(KeypadKey::Hash));
}
