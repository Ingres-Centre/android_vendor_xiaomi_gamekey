use crate::touch_emulator::TouchEmulator;
use chrono::Local;

mod log;
mod counter;
mod touch_emulator;

fn main() {
    let _ = log::log_info(
        "gamekeyd",
        format!(
            "Startup at {}...",
            Local::now().format("%d-%m-%Y %H:%M:%S")
        )
        .as_str(),
    );

    #[allow(unused)]
    let mut touch_emulator = TouchEmulator::new(2).unwrap();

    let _ = log::log_info(
        "gamekeyd",
        "Exiting..."
    );
}
