use jetbeep::lvgl::*;

pub unsafe fn app_main() {
    let scr = lv_screen_active();
    let label = lv_label_create(&scr);
    lv_label_set_text(&label, "service-menu: bootstrap app");
    lv_obj_center(&label);
}
