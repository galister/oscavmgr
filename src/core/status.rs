use std::{collections::VecDeque, sync::Arc, time::Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub struct StatusBar {
    messages: Vec<Arc<str>>,
    spinner: ProgressBar,
    send_counter: VecDeque<(f32, Instant)>,
    recv_counter: VecDeque<Instant>,
    fps_counter: VecDeque<Instant>,
    fps: f32,
    start: Instant,
    pub last_frame_time: f32,
}

impl StatusBar {
    pub fn new(multi: &MultiProgress) -> Self {
        let spinner = multi.add(ProgressBar::new_spinner());
        spinner.set_style(
            ProgressStyle::default_spinner().tick_chars("⠁⠂⠄⡀⡈⡐⡠⣀⣁⣂⣄⣌⣔⣤⣥⣦⣮⣶⣷⣿⡿⠿⢟⠟⡛⠛⠫⢋⠋⠍⡉⠉⠑⠡⢁"),
        );

        Self {
            messages: Vec::new(),
            spinner,
            send_counter: VecDeque::new(),
            recv_counter: VecDeque::new(),
            fps_counter: VecDeque::new(),
            start: Instant::now(),
            last_frame_time: 0f32,
            fps: 1f32,
        }
    }

    pub fn trip_fps_counter(&mut self) {
        if let Some(last) = self.fps_counter.back() {
            self.last_frame_time = last.elapsed().as_secs_f32();
        }
        self.fps_counter.push_back(Instant::now());

        while let Some(time) = self.fps_counter.front() {
            if time.elapsed().as_secs_f32() > 1. {
                self.fps_counter.pop_front();
            } else {
                break;
            }
        }

        let total_elapsed = self
            .fps_counter
            .front()
            .map(|time| time.elapsed().as_secs_f32())
            .unwrap_or(0f32);

        self.fps = self.fps_counter.len() as f32 / total_elapsed;
        self.add_item(format!("TICK:{:.0}/s", self.fps).into());
    }

    pub fn trip_recv_counter(&mut self) {
        self.recv_counter.push_back(Instant::now());
        while let Some(time) = self.recv_counter.front() {
            if time.elapsed().as_secs_f32() > 1. {
                self.recv_counter.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn recv_summary(&mut self) {
        let total_elapsed = self
            .recv_counter
            .front()
            .map(|time| time.elapsed().as_secs_f32())
            .unwrap_or(0f32);

        self.add_item(
            format!(
                "RECV:{:.0}/s",
                self.recv_counter.len() as f32 / total_elapsed
            )
            .into(),
        );
    }

    pub fn set_sent_count(&mut self, count: f32) {
        self.send_counter.push_back((count, Instant::now()));

        while let Some((_, time)) = self.send_counter.front() {
            if time.elapsed().as_secs_f32() > 1. {
                self.send_counter.pop_front();
            } else {
                break;
            }
        }

        let total_elapsed = self
            .send_counter
            .front()
            .map(|(_, time)| time.elapsed().as_secs_f32())
            .unwrap_or(0f32);

        let total = self
            .send_counter
            .iter()
            .map(|(count, _)| count)
            .sum::<f32>()
            / total_elapsed;

        self.add_item(format!("SEND:{:.1}/s", total).into());
    }

    pub fn add_item(&mut self, str: Arc<str>) {
        self.messages.push(str);
    }

    pub fn display(&mut self) {
        let uptime = self.start.elapsed().as_secs();
        if uptime >= 1 {
            let str = self.messages.join("  ");
            self.spinner.set_message(str);
        } else {
            self.spinner.set_message("Initializing...");
        }
        self.spinner.tick();
        self.messages.clear();
    }
}
