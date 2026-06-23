use core::time::Duration;

pub struct StaticPtr<T>(core::cell::UnsafeCell<*mut T>);

// SAFETY: single-threaded LVGL/workq usage only.
unsafe impl<T> Sync for StaticPtr<T> {}

impl<T> StaticPtr<T> {
    pub const fn new() -> Self {
        Self(core::cell::UnsafeCell::new(core::ptr::null_mut()))
    }

    pub fn set(&self, ptr: *mut T) {
        unsafe { *self.0.get() = ptr; }
    }

    pub fn get_mut(&self) -> Option<&mut T> {
        unsafe { (*self.0.get()).as_mut() }
    }
}

pub fn dispatch_to<T: 'static>(holder: &'static StaticPtr<T>, f: impl FnOnce(&mut T) + 'static) {
    jetbeep_core::workq::submit(Duration::from_secs(0), move |_| {
        if let Some(target) = holder.get_mut() {
            f(target);
        }
    });
}
