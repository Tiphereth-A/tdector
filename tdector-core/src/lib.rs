pub mod consts;
pub mod enums;
pub mod libs;

thread_local! {
    static IS_APP_DIRTY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn set_app_dirty(dirty: bool) {
    IS_APP_DIRTY.with(|flag| flag.set(dirty));
}

pub fn is_app_dirty() -> bool {
    IS_APP_DIRTY.with(std::cell::Cell::get)
}
