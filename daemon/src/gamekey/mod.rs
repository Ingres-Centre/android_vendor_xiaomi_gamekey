mod utils;

use crate::gamekey::utils::enumerate_devices;
use evdev_rs::enums::{EventCode, EV_KEY};
use evdev_rs::{Device, InputEvent, ReadFlag};
use nix::libc::EAGAIN;
use std::error::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::task;

#[derive(Debug)]
pub enum EventType {
    Open,
    Close,
    Press,
    Release,
}

#[derive(Debug)]
pub struct Event {
    pub event_type: EventType,
    pub slot: u32,
}

fn map_event(ev: InputEvent) -> Option<Event> {
    match ev.event_code {
        EventCode::EV_KEY(key) => match key {
            EV_KEY::KEY_F1 => Some(Event {
                slot: 0,
                event_type: if ev.value == 1 {
                    EventType::Press
                } else {
                    EventType::Release
                },
            }),
            EV_KEY::KEY_F2 => Some(Event {
                slot: 1,
                event_type: if ev.value == 1 {
                    EventType::Press
                } else {
                    EventType::Release
                },
            }),

            EV_KEY::KEY_F3 => {
                if ev.value == 1 {
                    Some(Event {
                        slot: 0,
                        event_type: EventType::Open,
                    })
                } else {
                    None
                }
            }
            EV_KEY::KEY_F4 => {
                if ev.value == 1 {
                    Some(Event {
                        slot: 0,
                        event_type: EventType::Close,
                    })
                } else {
                    None
                }
            }

            EV_KEY::KEY_F5 => {
                if ev.value == 1 {
                    Some(Event {
                        slot: 1,
                        event_type: EventType::Open,
                    })
                } else {
                    None
                }
            }
            EV_KEY::KEY_F6 => {
                if ev.value == 1 {
                    Some(Event {
                        slot: 1,
                        event_type: EventType::Close,
                    })
                } else {
                    None
                }
            }

            _ => None,
        },
        _ => None,
    }
}

pub fn read_gamekey_events() -> Result<Receiver<Event>, Box<dyn Error + Send + Sync>> {
    let (dev_path, _) = enumerate_devices()?
        .into_iter()
        .find(|(_, name)| name == "xm_gamekey")
        .ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Input device with name `xm_gamekey` not found",
        ))?;

    let device = Device::new_from_path(dev_path)?;

    let (tx, rx) = mpsc::channel::<Event>(4);
    task::spawn_blocking(move || {
        loop {
            let ev = match device.next_event(ReadFlag::BLOCKING) {
                Ok((_, ev)) => map_event(ev),
                Err(e) if e.raw_os_error() == Some(EAGAIN) => continue,
                Err(e) => panic!("Failed to poll event from gamekey device: {}", e),
            };

            if let Some(ev) = ev
                && let Err(e) = tx.blocking_send(ev)
            {
                eprintln!("{}", e);
                return;
            }
        }
    });

    Ok(rx)
}
