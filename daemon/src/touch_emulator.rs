use crate::counter::IncrementalCounter;
use anyhow::Context;
use evdev_rs::enums::{BusType, EventCode, EventType, InputProp, EV_ABS, EV_KEY, EV_SYN};
use evdev_rs::{AbsInfo, DeviceWrapper, EnableCodeData, InputEvent, UInputDevice, UninitDevice};
use std::fmt;
use std::fmt::Formatter;

pub struct TouchEmulator {
    udevice: UInputDevice,
    slot_states: Vec<bool>,
    touch_counter: IncrementalCounter<i32>,
}

#[derive(Debug)]
pub enum Error {
    InvalidSlotCount,
    InvalidSlotId,
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{err}"),
            Error::InvalidSlotCount => {
                write!(f, "Invalid slot count! Min count is 1, max 20.")
            }
            Error::InvalidSlotId => write!(f, "Invalid slot id!"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl TouchEmulator {
    pub fn new(slot_count: u8) -> anyhow::Result<Self, Error> {
        if slot_count == 0 || slot_count > 20 {
            return Err(Error::InvalidSlotCount);
        }

        let u = UninitDevice::new().unwrap();

        u.set_name("gamekey-touch");
        u.set_bustype(BusType::BUS_VIRTUAL as u16);
        u.set_vendor_id(0x6761); // ga
        u.set_product_id(0x6d65); // me

        u.enable(EventType::EV_KEY)?;
        u.enable(EventType::EV_ABS)?;
        u.enable_property(&InputProp::INPUT_PROP_DIRECT)?;

        u.enable(EventCode::EV_SYN(EV_SYN::SYN_REPORT))?;
        u.enable(EventCode::EV_SYN(EV_SYN::SYN_MT_REPORT))?;

        u.enable(EventCode::EV_KEY(EV_KEY::BTN_TOUCH))?;
        u.enable(EventCode::EV_KEY(EV_KEY::BTN_TOOL_FINGER))?;

        let abs = |min: i32, max: i32| {
            EnableCodeData::AbsInfo(AbsInfo {
                value: 0,
                minimum: min,
                maximum: max,
                flat: 0,
                fuzz: 0,
                resolution: 0,
            })
        };

        u.enable_event_code(
            &EventCode::EV_ABS(EV_ABS::ABS_MT_SLOT),
            Some(abs(0, (slot_count - 1) as i32)),
        )?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_TOUCH_MAJOR), Some(abs(0, 10800)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_TOUCH_MINOR), Some(abs(0, 24000)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_WIDTH_MAJOR), Some(abs(0, 127)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_WIDTH_MINOR), Some(abs(0, 127)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_ORIENTATION), Some(abs(-90, 90)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_POSITION_X), Some(abs(0, 10799)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_POSITION_Y), Some(abs(0, 23999)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_TRACKING_ID), Some(abs(0, 65535)))?;
        u.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_MT_DISTANCE), Some(abs(0, 127)))?;

        let mut slot_states = Vec::new();
        slot_states.resize(slot_count as usize, false);

        Ok(Self {
            udevice: UInputDevice::create_from_device(&u)
                .context("Failed to create UInputDevice from Device")?,
            slot_states,
            touch_counter: IncrementalCounter::new(0),
        })
    }

    fn tap(&mut self, slot: usize, pos: Option<(i32, i32)>) -> anyhow::Result<(), Error> {
        if self.slot_states.len() < slot {
            return Err(Error::InvalidSlotId);
        }

        let now = || std::time::SystemTime::now().try_into().unwrap();
        let is_press = pos.is_some();

        if self.slot_states[slot] ^ (!is_press) {
            return Ok(());
        }

        let touched_before = self.slot_states.iter().any(|s| *s);
        self.slot_states[slot] = is_press;
        let touched_after = self.slot_states.iter().any(|s| *s);

        self.udevice.write_event(&InputEvent {
            time: now(),
            event_code: EventCode::EV_ABS(EV_ABS::ABS_MT_SLOT),
            value: slot as i32,
        })?;

        self.udevice.write_event(&InputEvent {
            time: now(),
            event_code: EventCode::EV_ABS(EV_ABS::ABS_MT_TRACKING_ID),
            value: if is_press { self.touch_counter.next() } else { -1 },
        })?;

        if (is_press && !touched_before) || (!is_press && !touched_after) {
            self.udevice.write_event(&InputEvent {
                time: now(),
                event_code: EventCode::EV_KEY(EV_KEY::BTN_TOUCH),
                value: is_press as i32,
            })?;

            self.udevice.write_event(&InputEvent {
                time: now(),
                event_code: EventCode::EV_KEY(EV_KEY::BTN_TOOL_FINGER),
                value: is_press as i32,
            })?;
        }

        if let Some((x, y)) = pos {
            self.udevice.write_event(&InputEvent {
                time: now(),
                event_code: EventCode::EV_ABS(EV_ABS::ABS_MT_POSITION_X),
                value: x,
            })?;

            self.udevice.write_event(&InputEvent {
                time: now(),
                event_code: EventCode::EV_ABS(EV_ABS::ABS_MT_POSITION_Y),
                value: y,
            })?;
        }

        self.udevice.write_event(&InputEvent {
            time: now(),
            event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
            value: 0,
        })?;

        Ok(())
    }

    pub fn start_tap(&mut self, slot: usize, x: i32, y: i32) -> anyhow::Result<(), Error> {
        self.tap(slot, Some((x, y)))
    }

    pub fn stop_tap(&mut self, slot: usize) -> anyhow::Result<(), Error> {
        self.tap(slot, None)
    }
}
