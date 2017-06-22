use std::ffi::CString;
use std::os::raw::c_char;
use log::{self, Log, LogLevel, LogMetadata, SetLoggerError, LogRecord};

extern "C" {
    // These are the callbacks into C++, called from poll_smoltcp_stack.
    // No need to pass a module ID because opp keeps track of the "context module"
    // and will route the message accordingly.
    pub fn smoltcp_log_line(level: u8, text: *const c_char) -> ();
}

#[no_mangle]
pub unsafe extern "C" fn init_smoltcp_logging() {
    init_ev_logging().unwrap();
}

struct EVLogger;

impl Log for EVLogger {
    fn enabled(&self, _: &LogMetadata) -> bool {
        true
    }

    fn log(&self, record: &LogRecord) {
        let cs: CString = CString::new(format!("{}", record.args())).unwrap();

        unsafe {
            smoltcp_log_line(record.level() as u8, cs.as_ptr());
        }
    }
}

pub fn init_ev_logging() -> Result<(), SetLoggerError> {
    log::set_logger(|max_log_level| {

        max_log_level.set(LogLevel::Trace.to_log_level_filter());
        return Box::new(EVLogger);
    })
}
