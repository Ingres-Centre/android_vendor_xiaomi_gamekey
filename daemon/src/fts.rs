use crate::utils::udev::enumerate_devices;
use anyhow::Context;
use evdev_rs::{Device, InputEvent, ReadFlag};
use nix::errno::Errno;
use nix::ioctl_write_int;
use nix::libc::EAGAIN;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::fs::OpenOptions;
use std::os::fd::{AsFd, AsRawFd};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task;

ioctl_write_int!(eviocgrab, b'E', 0x90);

fn working_thread(device: Device, tx: Sender<InputEvent>) {
    let fd = device.file().as_fd();
    let mut pfd = [PollFd::new(fd, PollFlags::POLLIN)];

    loop {
        if let Err(e) = poll(&mut pfd, PollTimeout::NONE) {
            match e {
                Errno::EINTR => continue,
                e => panic!("poll: {e}"),
            }
        }

        loop {
            let ev = match device.next_event(ReadFlag::BLOCKING) {
                Ok((_, ev)) => ev,
                Err(e) if e.raw_os_error() == Some(EAGAIN) => break,
                Err(e) => panic!("Failed to poll event from fts device: {}", e),
            };

            if let Err(e) = tx.blocking_send(ev) {
                eprintln!("{}", e);
                return;
            }
        }
    }
}

pub fn read_fts_events() -> anyhow::Result<Receiver<InputEvent>> {
    let (dev_path, _) = enumerate_devices()
        .context("Failed to enumerate devices")?
        .into_iter()
        .find(|(_, name)| name == "fts")
        .ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Input device with name `fts` not found",
        ))?;

    let file = OpenOptions::new()
        .read(true)
        .open(dev_path)
        .context("Failed to open device")?;
    let fd = file.as_raw_fd();

    unsafe {
        eviocgrab(fd, 1).context("Failed to grab device")?;
    }

    let device = Device::new_from_file(file).context("Failed to create Device from File")?;
    let (tx, rx) = mpsc::channel::<InputEvent>(4);

    task::spawn_blocking(move || working_thread(device, tx));

    Ok(rx)
}
