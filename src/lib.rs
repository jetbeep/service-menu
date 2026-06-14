#![no_std]
extern crate alloc;

#[cfg(test)]
extern crate std;

mod core;
mod ui;

use alloc::boxed::Box;
use ::core::time::Duration;

use futures::StreamExt;
use jetbeep::{bus, executor, workq};

use crate::core::{
    app::App,
    app_dispatcher::{bind_app, post_app_event},
    event::AppEvent,
};

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

pub unsafe fn app_main() {
    log::info!("start service-menu app");

    let app = Box::leak(Box::new(App::new()));
    app.render();
    bind_app(app);

    executor::run(clock_tick_task());

    #[cfg(feature = "simulator")]
    executor::run(keypad_forward_task());
}
