use crate::gamekey::read_gamekey_events;
use anyhow::Context;
use gamekey::EventType;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use touch_emulator::TouchEmulator;

use crate::fts::read_fts_events;
use crate::touch_merger::{TouchMerger, TouchSourceDeclaration};
#[cfg(not(feature = "local"))]
use {
    crate::binder_service::SettingsService, binder_tokio::TokioRuntime,
    gamekeyd_aidl::aidl::org::ingres::gamekeys::ISettingsService::BnSettingsService,
    gamekeyd_aidl::binder::BinderFeatures, log::LevelFilter, std::error::Error,
};

mod gamekey;

#[cfg(not(feature = "local"))]
mod binder_service;

mod fts;
mod touch_emulator;
mod touch_merger;
mod utils;

pub type GameKeyData = Option<(i32, i32)>;

pub struct GameKeyCompound {
    pub upper: GameKeyData,
    pub lower: GameKeyData,
}

pub struct Controller {
    pub data: RwLock<GameKeyCompound>,
}

fn main() {
    #[cfg(not(feature = "local"))]
    let _init_success = logger::init(
        logger::Config::default()
            .with_tag_on_device("gamekeyd")
            .with_max_level(LevelFilter::Trace),
    );

    #[cfg(feature = "local")]
    {
        unsafe {
            std::env::set_var("RUST_LOG", "debug");
        }
        env_logger::init();
    }

    log::info!("Startup...");

    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        if let Err(e) = async_main().await {
            log::error!("{:?}", e);
        }
    })
}

async fn gk_event_loop(
    mut touch_emulator: TouchEmulator,
    compound: Arc<RwLock<GameKeyCompound>>,
) -> anyhow::Result<()> {
    let mut event_stream = read_gamekey_events().context("Get gk event stream failed")?;

    let mut last_open_time: [SystemTime; 2] = [SystemTime::UNIX_EPOCH, SystemTime::UNIX_EPOCH];
    let mut last_close_time: [SystemTime; 2] = [SystemTime::UNIX_EPOCH, SystemTime::UNIX_EPOCH];

    loop {
        let ev = event_stream.recv().await;

        if let Some(ev) = ev {
            match &ev.r#type {
                EventType::Close => {
                    let opposite_close_at = *last_close_time.get((ev.slot ^ 1) as usize).unwrap();
                    let current_close_at = last_close_time.get_mut(ev.slot as usize).unwrap();

                    *current_close_at = SystemTime::now();

                    if current_close_at.duration_since(opposite_close_at)? < Duration::from_secs(1)
                    {
                        #[cfg(not(feature = "local"))]
                        rustutils::system_properties::write("vendor.gamekeyd.both_state", "0")
                            .unwrap();
                        log::debug!("Both triggers are closed!");
                    }
                }
                EventType::Open => {
                    let opposite_open_at = *last_open_time.get((ev.slot ^ 1) as usize).unwrap();
                    let current_open_at = last_open_time.get_mut(ev.slot as usize).unwrap();

                    *current_open_at = SystemTime::now();

                    if current_open_at.duration_since(opposite_open_at)? < Duration::from_secs(1) {
                        #[cfg(not(feature = "local"))]
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

                    #[allow(clippy::collapsible_if)]
                    if let Some((x, y)) = data {
                        if let Err(e) = touch_emulator.start_tap(ev.slot as usize, x, y).await {
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

                    #[allow(clippy::collapsible_if)]
                    if data.is_some() {
                        if let Err(e) = touch_emulator.stop_tap(ev.slot as usize).await {
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
    let compound = Arc::new(RwLock::new(
        #[cfg(not(feature = "local"))]
        GameKeyCompound {
            lower: None,
            upper: None,
        },
        #[cfg(feature = "local")]
        GameKeyCompound {
            lower: Some((1000, 2000)),
            upper: Some((3000, 6000)),
        },
    ));

    log::info!("hi probably?");

    #[cfg(not(feature = "local"))]
    {
        binder::ProcessState::start_thread_pool();
        log::info!("Binder thread pool has been started!");

        let name = "org.ingres.gamekeys.ISettingsService/default";
        let svc = BnSettingsService::new_async_binder(
            SettingsService::new(compound.clone()),
            binder_tokio::TokioRuntime(tokio::runtime::Handle::current()),
            BinderFeatures::default(),
        );
        binder::add_service(name, svc.as_binder())
            .context("Failed to register ISettingsService")?;
        log::info!("Binder service '{}' registered successfully!", name);
    }

    let (touch_emulator, touch_emulator_rx) =
        TouchEmulator::new(2).context("Failed to create touch emulator")?;

    let fts_rx = read_fts_events().context("Failed to get fts input event stream")?;

    let touch_merger = TouchMerger::new(Box::from([
        (TouchSourceDeclaration::new(10), fts_rx),
        (TouchSourceDeclaration::new(2), touch_emulator_rx),
    ]))
    .context("Failed to create Touch Merger")?;

    tokio::select! {
        res = touch_merger.processing_task() => {
            if res.is_err() {
                log::error!("An error occurred while merging input events: {:#?}", res);
            }
        }
        res = gk_event_loop(touch_emulator, compound.clone()) => {
            if res.is_err() {
                log::error!("An error occurred while reading input events from gamekey device: {:#?}", res);
            }
        }
    };

    Ok(())
}
