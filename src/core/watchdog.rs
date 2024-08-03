use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::Instant,
};

pub struct Watchdog {
    start: Instant,
    self_drive: Arc<AtomicBool>,
    last_received: Arc<AtomicU64>,
}

impl Watchdog {
    pub fn new(self_drive: Arc<AtomicBool>) -> Self {
        Self {
            start: Instant::now(),
            self_drive,
            last_received: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn update(&self) {
        self.last_received
            .store(self.start.elapsed().as_millis() as _, Ordering::Relaxed);
    }

    pub fn run(&self) {
        let sleep_duration = std::time::Duration::from_secs(1);
        let self_drive = self.self_drive.clone();
        let last_received = self.last_received.clone();
        let start = self.start;

        thread::spawn(move || loop {
            let last_recv_time = last_received.load(std::sync::atomic::Ordering::Relaxed);

            let elapsed = start.elapsed().as_millis() as u64;
            if elapsed - last_recv_time > 500 {
                self_drive.store(true, Ordering::Relaxed);
            }
            thread::sleep(sleep_duration);
        });
    }
}
