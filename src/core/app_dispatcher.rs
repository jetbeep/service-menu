use crate::core::{
    app::App,
    event::AppEvent,
    static_ptr::{dispatch_to, StaticPtr},
};

static APP: StaticPtr<App> = StaticPtr::new();

pub fn bind_app(app: &'static mut App) {
    APP.set(app);
}

pub fn post_app_event(event: AppEvent) {
    dispatch_to(&APP, move |app| app.handle_event(event));
}
