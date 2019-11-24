use crate::constants;

#[cfg(target_os = "windows")]
use super::windows::show_error_dialog;

pub trait ResultExtensions<T, E: std::fmt::Debug> {
    fn unwrap_or_fail_fast(self, msg: &str) -> T;
}

pub trait OptionExtensions<T> {
    fn unwrap_or_fail_fast(self, msg: &str) -> T;
}

impl<T, E: std::fmt::Debug> ResultExtensions<T, E> for Result<T, E> {
    fn unwrap_or_fail_fast(self, msg: &str) -> T {
        match self {
            Ok(t) => t,
            Err(e) => {
                fail_fast(&format!("{}\n Error: {:?}", msg, &e))
            },
        }
    }
}

impl<T> OptionExtensions<T> for Option<T> {
    fn unwrap_or_fail_fast(self, msg: &str) -> T {
        match self {
            Some(t) => t,
            None => {
                fail_fast(msg)
            }
        }
    }
}

pub fn fail_fast(msg: &str) -> ! {
    show_error_dialog(constants::STR_SORRY_DIALOG_TITLE, msg);
    panic!("Fatal error: {}", msg)
}
