use crate::gamekey::read_gamekey_events;
use crate::touch_emulator::TouchEmulator;
use chrono::Local;
use std::error::Error;
use tokio::runtime::Runtime;

mod counter;
mod gamekey;
mod log;
mod touch_emulator;

fn main() {
    let _ = log::log_info(
        "gamekeyd",
        format!("Startup at {}...", Local::now().format("%d-%m-%Y %H:%M:%S")).as_str(),
    );

    let rt = Runtime::new().unwrap();
    rt.block_on(async_main()).unwrap();
}

async fn async_main() -> Result<(), Box<dyn Error + Send + Sync>> {
    #[allow(unused)]
    let mut touch_emulator = TouchEmulator::new(2)?;
    let mut event_stream = read_gamekey_events()?;

    loop {
        let ev = event_stream.recv().await;

        if let Some(ev) = ev {
            println!("Event: {:#?}", ev);
            continue;
        }
    }
}
