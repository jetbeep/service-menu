#![no_std]
extern crate alloc;

#[cfg(test)]
extern crate std;

mod core;
mod device_settings_cache;
mod ui;

use alloc::boxed::Box;
use ::core::time::Duration;

use futures::StreamExt;
use jetbeep::{bus, executor, workq};
use jetbeep::proto::keypad::ButtonType;

use crate::core::{
    app::App,
    app_dispatcher::{bind_app, post_app_event},
    event::AppEvent,
};

fn keypad_has_letters(settings: &jetbeep::proto::DeviceSettings) -> bool {
    settings
        .keypad_settings
        .as_ref()
        .map(|k| {
            k.alphabet.iter().any(|v| {
                *v == ButtonType::A as i32
                    || *v == ButtonType::B as i32
                    || *v == ButtonType::C as i32
                    || *v == ButtonType::D as i32
            })
        })
        .unwrap_or(false)
}

#[cfg(feature = "simulator")]
async fn keypad_forward_task() {
    // Ensure this app owns the live keypad stream sender for this run.
    bus::keypad_unsubscribe();
    let mut rx = bus::keypad_subscribe();
    while let Some(key) = rx.next().await {
        post_app_event(AppEvent::Keypad(key));
    }
}

async fn clock_tick_task() {
    loop {
        workq::delay(Duration::from_secs(1)).await;
        post_app_event(AppEvent::Tick);
    }
}

async fn load_keypad_layout_task() {
    if let Some(settings) = device_settings_cache::get_cached() {
        post_app_event(AppEvent::KeypadLayoutLoaded(keypad_has_letters(&settings)));
        return;
    }

    if device_settings_cache::begin_load_once() {
        match bus::get_device_settings().await {
            Ok(settings) => {
                let has_letters = keypad_has_letters(&settings);
                device_settings_cache::set_loaded(settings);
                post_app_event(AppEvent::KeypadLayoutLoaded(has_letters));
            }
            Err(err) => {
                log::warn!("device settings unavailable, defaulting keypad to 3x4: {}", err);
                device_settings_cache::set_failed();
                post_app_event(AppEvent::KeypadLayoutLoaded(false));
            }
        }
        return;
    }

    let has_letters = device_settings_cache::wait_loaded()
        .await
        .as_ref()
        .map(keypad_has_letters)
        .unwrap_or(false);

    post_app_event(AppEvent::KeypadLayoutLoaded(has_letters));
}

pub unsafe fn app_main() {
    log::info!("start service-menu app");

    let app = Box::leak(Box::new(App::new()));
    app.render();
    bind_app(app);

    executor::run(clock_tick_task());
    executor::run(load_keypad_layout_task());

    #[cfg(feature = "simulator")]
    executor::run(keypad_forward_task());
}
