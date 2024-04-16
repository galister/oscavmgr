use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
};

pub struct Watchdog {
    self_drive: Arc<AtomicBool>,
    last_received: Arc<AtomicU64>,
}

impl Watchdog {
    pub fn new(self_drive: Arc<AtomicBool>) -> Self {
        Self {
            self_drive,
            last_received: Arc::new(AtomicU64::new(now())),
        }
    }

    pub fn update(&self) {
        self.last_received.store(now(), Ordering::Relaxed);
    }

    pub fn run(&self) {
        let sleep_duration = std::time::Duration::from_secs(1);
        let self_drive = self.self_drive.clone();
        let last_received = self.last_received.clone();

        thread::spawn(move || loop {
            let last_recv_time = last_received.load(std::sync::atomic::Ordering::Relaxed);
            if now() - last_recv_time > 5 {
                self_drive.store(true, Ordering::Relaxed);
            }
            thread::sleep(sleep_duration);
        });
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap() // safe to unwrap
        .as_secs()
}
