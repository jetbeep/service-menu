#![no_std]

use jetbeep::lvgl::prelude::*;

pub unsafe fn app_main() {
    let screen = Screen::active();
    Label::new(&screen)
        .align(LvAlign::Center, 0, 0)
        .text("service-menu: bootstrap app");
}
