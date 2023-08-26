use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use wasm_bindgen::{prelude::*, JsCast};

const MAX_RESOLUTION: (u32, u32) = (2048, 2048);

pub fn window_size() -> (u32, u32) {
    if let Some(window) = web_sys::window() {
        (
            (window.inner_width().unwrap().as_f64().unwrap() as u32).min(MAX_RESOLUTION.0),
            (window.inner_height().unwrap().as_f64().unwrap() as u32).min(MAX_RESOLUTION.1),
        )
    } else {
        (800, 600)
    }
}

pub fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

pub fn document() -> web_sys::Document {
    window().document().expect("no global `document` exists")
}

pub fn get_element_by_id(id: &str) -> web_sys::HtmlElement {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("Element '{id:?}' does not exist"))
        .dyn_into::<web_sys::HtmlElement>()
        .unwrap_or_else(|_| panic!("Could not dyn element '{id:?}' into HtmlElement"))
}

pub fn canvas_element() -> web_sys::HtmlElement {
    get_element_by_id("out_canvas")
}

pub fn is_pointer_locked() -> bool {
    document().pointer_lock_element().is_some()
}

pub fn request_pointer_lock() {
    canvas_element().request_pointer_lock()
}

pub fn exit_pointer_lock() {
    document().exit_pointer_lock()
}

pub fn register_mouse_click(running: Arc<AtomicBool>) {
    let closure = Closure::<dyn Fn()>::new(move || {
        if running.load(Ordering::Relaxed) {
            request_pointer_lock();
        }
    });

    canvas_element()
        .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
        .unwrap();

    closure.forget();
}

pub fn register_window_resize(window_resized: Arc<AtomicBool>) {
    let closure = Closure::<dyn Fn()>::new(move || window_resized.store(true, Ordering::Relaxed));

    window()
        .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
        .unwrap();

    closure.forget();
}
