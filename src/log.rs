use std::ffi::{CString, NulError};
use std::fmt::Formatter;
use std::os::raw::{c_char, c_int};

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

#[derive(Debug, Clone)]
pub enum LogError {
    Nul(NulError),
    RetCode(c_int),
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LogError::Nul(err) => write!(f, "{}", err),
            LogError::RetCode(code) => write!(f, "Invalid return code {}!", code),
        }
    }
}

impl std::error::Error for LogError {}

impl From<NulError> for LogError {
    fn from(err: NulError) -> LogError {
        LogError::Nul(err)
    }
}

impl From<c_int> for LogError {
    fn from(err: c_int) -> LogError {
        LogError::RetCode(err)
    }
}

pub fn log_info(tag: &str, message: &str) -> Result<(), LogError> {
    #[cfg(debug_assertions)]
    println!("[{}] {}", tag, message);

    use std::os::raw::c_int;

    #[repr(i32)]
    enum LogPriority {
        Info = 4,
    }

    let tag = CString::new(tag)?;
    let message = CString::new(message)?;

    let rc =
        unsafe { __android_log_write(LogPriority::Info as c_int, tag.as_ptr(), message.as_ptr()) };

    if rc < 0 {
        return Err(rc.into());
    }

    Ok(())
}
