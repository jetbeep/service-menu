use crate::core::{
    app::App,
    event::AppEvent,
    static_ptr::{dispatch_to, StaticPtr},
};

static APP: StaticPtr<App> = StaticPtr::new();

pub fn bind_app(app: &'static mut App) {
    APP.set(app);
}

/// Detach the app from the dispatcher so in-flight callbacks become no-ops.
/// Used by the soft-kill path.
pub fn unbind_app() {
    APP.clear();
}

pub fn post_app_event(event: AppEvent) {
    dispatch_to(&APP, move |app| app.handle_event(event));
}
