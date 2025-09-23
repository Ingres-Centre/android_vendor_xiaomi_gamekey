use crate::gamekey::read_gamekey_events;
use crate::server::SettingsService;
use anyhow::Context;
use binder_tokio::TokioRuntime;
use gamekey::EventType;
use log::LevelFilter;
use gamekeyd_aidl::aidl::org::ingres::gamekeys::ISettingsService::BnSettingsService;
use gamekeyd_aidl::binder::BinderFeatures;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use touch_emulator::TouchEmulator;

mod counter;
mod gamekey;
mod server;
mod touch_emulator;

pub type GameKeyData = Option<(i32, i32)>;

pub struct GameKeyCompound {
    pub upper: GameKeyData,
    pub lower: GameKeyData,
}

pub struct Controller {
    pub data: RwLock<GameKeyCompound>,
}

fn main() {
    let _init_success = logger::init(
        logger::Config::default().with_tag_on_device("gamekeyd").with_max_level(LevelFilter::Trace),
    );

    log::info!("Startup...");

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        if let Err(e) = async_main().await {
            log::error!("{:?}", e);
        }
    })
}

async fn gk_event_loop(compound: Arc<RwLock<GameKeyCompound>>) -> anyhow::Result<()> {
    let mut touch_emulator = TouchEmulator::new(2).context("Failed to create touch emulator")?;
    let mut event_stream = read_gamekey_events().context("Get gk event stream failed")?;

    let mut last_open_time: [std::time::SystemTime; 2] =
        [std::time::SystemTime::UNIX_EPOCH, std::time::SystemTime::UNIX_EPOCH];
    let mut last_close_time: [std::time::SystemTime; 2] =
        [std::time::SystemTime::UNIX_EPOCH, std::time::SystemTime::UNIX_EPOCH];

    loop {
        let ev = event_stream.recv().await;

        if let Some(ev) = ev {
            match &ev.r#type {
                EventType::Close => {
                    let opposite_close_at =
                        last_close_time.get((ev.slot ^ 1) as usize).unwrap().clone();
                    let current_close_at = last_close_time.get_mut(ev.slot as usize).unwrap();

                    *current_close_at = SystemTime::now();

                    if (current_close_at.duration_since(opposite_close_at).unwrap()
                        < Duration::from_secs(1))
                    {
                        rustutils::system_properties::write("vendor.gamekeyd.both_state", "0")
                            .unwrap();
                        log::debug!("Both triggers are closed!");
                    }
                }
                EventType::Open => {
                    let opposite_open_at =
                        last_open_time.get((ev.slot ^ 1) as usize).unwrap().clone();
                    let current_open_at = last_open_time.get_mut(ev.slot as usize).unwrap();

                    *current_open_at = SystemTime::now();

                    if (current_open_at.duration_since(opposite_open_at).unwrap()
                        < Duration::from_secs(1))
                    {
                        rustutils::system_properties::write("vendor.gamekeyd.both_state", "1")
                            .unwrap();
                        log::debug!("Both triggers are opened!");
                    }
                }
                EventType::Press => {
                    let compound_lock = compound.read().await;

                    let data = match ev.slot {
                        0 => compound_lock.upper,
                        1 => compound_lock.lower,
                        _ => unreachable!(),
                    };

                    if let Some((x, y)) = data {
                        if let Err(e) = touch_emulator.start_tap(ev.slot as usize, x, y) {
                            log::warn!("Failed to start tap at {}, {} in slot {}!", x, y, ev.slot);
                            log::warn!("{}", e);
                        }
                    }
                }
                EventType::Release => {
                    let compound_lock = compound.read().await;

                    let data = match ev.slot {
                        0 => compound_lock.upper,
                        1 => compound_lock.lower,
                        _ => unreachable!(),
                    };

                    if let Some((x, y)) = data {
                        if let Err(e) = touch_emulator.stop_tap(ev.slot as usize) {
                            log::warn!("Failed to stop tap in slot {}!", ev.slot);
                            log::warn!("{}", e);
                        }
                    }
                }
            }

            log::debug!("Event: {:#?}", ev);
            continue;
        } else {
            log::warn!("GameKey event stream is dead!");
            break;
        }
    }

    Ok(())
}

async fn async_main() -> anyhow::Result<()> {
    let compound = Arc::new(RwLock::new(GameKeyCompound { lower: None, upper: None }));

    log::info!("hi probably?");

    binder::ProcessState::start_thread_pool();
    log::info!("Binder thread pool has been started!");

    let name = "org.ingres.gamekeys.ISettingsService/default";
    let svc = BnSettingsService::new_async_binder(
        SettingsService::new(compound.clone()),
        binder_tokio::TokioRuntime(tokio::runtime::Handle::current()),
        BinderFeatures::default(),
    );
    binder::add_service(name, svc.as_binder()).context("Failed to register ISettingsService")?;
    log::info!("Binder service '{}' registered successfully!", name);

    log::info!("Pooling gamekey events...");
    gk_event_loop(compound.clone()).await?;

    Ok(())
}
