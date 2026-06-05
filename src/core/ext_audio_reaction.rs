use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use colored::{Color, Colorize};
use once_cell::sync::Lazy;
use rosc::{OscBundle, OscType};

use super::{bundle::AvatarBundle, AppState};

const DIRECTION_PARAM: &str = "audio_direction";
const VOLUME_PARAM: &str = "audio_volume";
const STALE_AFTER: Duration = Duration::from_millis(500);
const SILENCE_VOLUME: f32 = 0.0001;

static AUD_ON: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "AUD:ON".color(Color::Green)).into());
static AUD_WAIT: Lazy<Arc<str>> =
    Lazy::new(|| format!("{}", "AUD:WAIT".color(Color::BrightBlack)).into());
static AUD_ERR: Lazy<Arc<str>> = Lazy::new(|| format!("{}", "AUD:ERR".color(Color::Red)).into());

pub struct ExtAudioReaction {
    enabled: bool,
    shared: Arc<Mutex<SharedAudio>>,
}

impl ExtAudioReaction {
    pub fn new(enabled: bool) -> Self {
        let shared = Arc::new(Mutex::new(SharedAudio::default()));

        if enabled {
            start_worker(shared.clone());
        }

        Self { enabled, shared }
    }

    pub fn step(&mut self, state: &mut AppState, bundle: &mut OscBundle) {
        if !self.enabled {
            return;
        }

        let mut status = AudioStatus::Waiting;
        let mut reading = AudioReading::default();

        if let Ok(shared) = self.shared.try_lock() {
            let fresh = shared
                .last_update
                .is_some_and(|updated| updated.elapsed() <= STALE_AFTER);

            status = match (shared.status, fresh) {
                (AudioStatus::On, true) => {
                    reading = AudioReading {
                        direction: shared.direction,
                        volume: shared.volume,
                    };
                    AudioStatus::On
                }
                (AudioStatus::Error, _) => AudioStatus::Error,
                _ => AudioStatus::Waiting,
            };
        }

        state.status.add_item(status_label(status));
        bundle.send_parameter(DIRECTION_PARAM, OscType::Float(reading.direction));
        bundle.send_parameter(VOLUME_PARAM, OscType::Float(reading.volume));
    }
}

fn status_label(status: AudioStatus) -> Arc<str> {
    match status {
        AudioStatus::On => AUD_ON.clone(),
        AudioStatus::Waiting => AUD_WAIT.clone(),
        AudioStatus::Error => AUD_ERR.clone(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AudioStatus {
    Waiting,
    On,
    Error,
}

struct SharedAudio {
    status: AudioStatus,
    direction: f32,
    volume: f32,
    last_update: Option<Instant>,
}

impl Default for SharedAudio {
    fn default() -> Self {
        Self {
            status: AudioStatus::Waiting,
            direction: 0.0,
            volume: 0.0,
            last_update: None,
        }
    }
}

fn set_status(shared: &Arc<Mutex<SharedAudio>>, status: AudioStatus) {
    if let Ok(mut shared) = shared.lock() {
        shared.status = status;
    }
}

fn set_error(shared: &Arc<Mutex<SharedAudio>>) {
    if let Ok(mut shared) = shared.lock() {
        shared.status = AudioStatus::Error;
        shared.direction = 0.0;
        shared.volume = 0.0;
        shared.last_update = None;
    }
}

fn set_reading(shared: &Arc<Mutex<SharedAudio>>, reading: AudioReading) {
    if let Ok(mut shared) = shared.lock() {
        shared.status = AudioStatus::On;
        shared.direction = reading.direction;
        shared.volume = reading.volume;
        shared.last_update = Some(Instant::now());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct AudioReading {
    direction: f32,
    volume: f32,
}

#[derive(Default)]
struct SmoothedReading {
    direction: f32,
    volume: f32,
    initialized: bool,
}

impl SmoothedReading {
    fn update(&mut self, raw: AudioReading) -> AudioReading {
        const DIRECTION_SMOOTHING: f32 = 0.25;
        const VOLUME_SMOOTHING: f32 = 0.35;

        let target_direction = if raw.volume <= SILENCE_VOLUME {
            0.0
        } else {
            raw.direction
        };

        if self.initialized {
            self.direction += (target_direction - self.direction) * DIRECTION_SMOOTHING;
            self.volume += (raw.volume - self.volume) * VOLUME_SMOOTHING;
        } else {
            self.direction = target_direction;
            self.volume = raw.volume;
            self.initialized = true;
        }

        AudioReading {
            direction: self.direction.clamp(-1.0, 1.0),
            volume: self.volume.clamp(0.0, 1.0),
        }
    }
}

fn analyze_f32_samples(samples: &[f32], channels: usize) -> AudioReading {
    let mut sums = AudioSums::new(channels);

    for (sample_index, sample) in samples.iter().copied().enumerate() {
        sums.add(sample_index, sample);
    }

    sums.finish()
}

#[derive(Default)]
struct AudioSums {
    channels: usize,
    left_sq: f32,
    left_count: usize,
    right_sq: f32,
    right_count: usize,
    all_sq: f32,
    all_count: usize,
}

impl AudioSums {
    fn new(channels: usize) -> Self {
        Self {
            channels,
            ..Default::default()
        }
    }

    fn add(&mut self, sample_index: usize, sample: f32) {
        if self.channels == 0 {
            return;
        }

        let sample = if sample.is_finite() { sample } else { 0.0 };
        let square = sample * sample;
        let channel = sample_index % self.channels;

        self.all_sq += square;
        self.all_count += 1;

        if channel == 0 {
            self.left_sq += square;
            self.left_count += 1;
        } else if channel == 1 {
            self.right_sq += square;
            self.right_count += 1;
        }
    }

    fn finish(self) -> AudioReading {
        if self.channels == 0 || self.all_count == 0 {
            return AudioReading::default();
        }

        let volume = (self.all_sq / self.all_count as f32).sqrt().clamp(0.0, 1.0);
        if self.channels < 2 || volume <= SILENCE_VOLUME {
            return AudioReading {
                direction: 0.0,
                volume,
            };
        }

        let left_rms = if self.left_count == 0 {
            0.0
        } else {
            (self.left_sq / self.left_count as f32).sqrt()
        };
        let right_rms = if self.right_count == 0 {
            0.0
        } else {
            (self.right_sq / self.right_count as f32).sqrt()
        };

        let sum = left_rms + right_rms;
        let direction = if sum <= f32::EPSILON {
            0.0
        } else {
            ((right_rms - left_rms) / sum).clamp(-1.0, 1.0)
        };

        AudioReading { direction, volume }
    }
}

#[cfg(target_os = "linux")]
fn start_worker(shared: Arc<Mutex<SharedAudio>>) {
    let builder = thread::Builder::new().name("oscavmgr-audio-reaction".to_string());
    if let Err(error) = builder.spawn({
        let shared = shared.clone();
        move || {
            if let Err(error) = pipewire_capture::run(shared.clone()) {
                log::warn!("PipeWire audio reaction stopped: {error:?}");
                set_error(&shared);
            }
        }
    }) {
        log::warn!("Failed to start audio reaction worker: {error}");
        set_error(&shared);
    }
}

#[cfg(not(target_os = "linux"))]
fn start_worker(shared: Arc<Mutex<SharedAudio>>) {
    log::warn!("--audio-reaction requires Linux PipeWire support on this build");
    set_error(&shared);
}

#[cfg(target_os = "linux")]
mod pipewire_capture {
    use std::{cell::RefCell, convert::TryInto, mem, rc::Rc, sync::Arc};

    use anyhow::{Context, Result};
    use pipewire as pw;
    use pw::{properties::properties, spa};
    use serde_json::Value;
    use spa::param::format::{MediaSubtype, MediaType};
    use spa::param::format_utils;
    use spa::pod::Pod;

    use super::{
        set_error, set_reading, set_status, AudioReading, AudioStatus, SharedAudio, SmoothedReading,
    };

    const DEFAULT_AUDIO_SINK: &str = "default.audio.sink";
    const CONFIGURED_AUDIO_SINK: &str = "default.configured.audio.sink";

    pub fn run(shared: Arc<std::sync::Mutex<SharedAudio>>) -> Result<()> {
        pw::init();

        let mainloop = pw::main_loop::MainLoopRc::new(None).context("create PipeWire main loop")?;
        let context =
            pw::context::ContextRc::new(&mainloop, None).context("create PipeWire context")?;
        let core = context
            .connect_rc(None)
            .context("connect to PipeWire core")?;
        let registry = core.get_registry_rc().context("get PipeWire registry")?;

        set_status(&shared, AudioStatus::Waiting);

        let runtime = Rc::new(RefCell::new(AudioRuntime::new(core, shared.clone())));
        runtime.borrow_mut().reconnect(None);

        let registry_listener = registry
            .add_listener_local()
            .global({
                let registry = registry.clone();
                let shared = shared.clone();
                let runtime = runtime.clone();
                move |global| {
                    let object = global.to_owned();
                    if !is_default_metadata(&object) {
                        return;
                    }

                    if runtime.borrow().metadata_id == Some(object.id) {
                        return;
                    }

                    let metadata = match registry.bind::<pw::metadata::Metadata, _>(&object) {
                        Ok(metadata) => metadata,
                        Err(error) => {
                            log::warn!("Failed to bind PipeWire default metadata: {error}");
                            set_error(&shared);
                            return;
                        }
                    };

                    let metadata_listener = metadata
                        .add_listener_local()
                        .property({
                            let runtime = runtime.clone();
                            move |_subject, key, _type_, value| {
                                runtime.borrow_mut().metadata_property(key, value);
                                0
                            }
                        })
                        .register();

                    let mut runtime = runtime.borrow_mut();
                    runtime.metadata_id = Some(object.id);
                    runtime.metadata_listener = Some(metadata_listener);
                    runtime.metadata = Some(metadata);
                }
            })
            .register();

        let _keep_alive = registry_listener;
        mainloop.run();
        Ok(())
    }

    fn is_default_metadata(
        object: &pw::registry::GlobalObject<pw::properties::PropertiesBox>,
    ) -> bool {
        object.type_ == pw::types::ObjectType::Metadata
            && object
                .props
                .as_ref()
                .and_then(|props| props.get("metadata.name"))
                == Some("default")
    }

    struct AudioRuntime {
        core: pw::core::CoreRc,
        shared: Arc<std::sync::Mutex<SharedAudio>>,
        default_sink: Option<String>,
        configured_sink: Option<String>,
        current_target: Option<String>,
        stream: Option<StreamConnection>,
        metadata_id: Option<u32>,
        metadata_listener: Option<pw::metadata::MetadataListener>,
        metadata: Option<pw::metadata::Metadata>,
    }

    impl AudioRuntime {
        fn new(core: pw::core::CoreRc, shared: Arc<std::sync::Mutex<SharedAudio>>) -> Self {
            Self {
                core,
                shared,
                default_sink: None,
                configured_sink: None,
                current_target: None,
                stream: None,
                metadata_id: None,
                metadata_listener: None,
                metadata: None,
            }
        }

        fn metadata_property(&mut self, key: Option<&str>, value: Option<&str>) {
            match key {
                Some(DEFAULT_AUDIO_SINK) => self.default_sink = value.and_then(parse_sink_name),
                Some(CONFIGURED_AUDIO_SINK) => {
                    self.configured_sink = value.and_then(parse_sink_name)
                }
                None => {
                    self.default_sink = None;
                    self.configured_sink = None;
                }
                _ => return,
            }

            self.reconnect(self.desired_target());
        }

        fn desired_target(&self) -> Option<String> {
            self.default_sink
                .clone()
                .or_else(|| self.configured_sink.clone())
        }

        fn reconnect(&mut self, target: Option<String>) {
            if self.current_target == target && self.stream.is_some() {
                return;
            }

            self.stream = None;
            self.current_target = target.clone();
            set_status(&self.shared, AudioStatus::Waiting);

            match connect_stream(&self.core, target.as_deref(), self.shared.clone()) {
                Ok(stream) => self.stream = Some(stream),
                Err(error) => {
                    log::warn!("Failed to connect PipeWire audio capture stream: {error:?}");
                    set_error(&self.shared);
                }
            }
        }
    }

    struct StreamConnection {
        listener: pw::stream::StreamListener<AudioProcessState>,
        stream: pw::stream::StreamRc,
    }

    struct AudioProcessState {
        format: spa::param::audio::AudioInfoRaw,
        shared: Arc<std::sync::Mutex<SharedAudio>>,
        smoothing: SmoothedReading,
    }

    fn connect_stream(
        core: &pw::core::CoreRc,
        target: Option<&str>,
        shared: Arc<std::sync::Mutex<SharedAudio>>,
    ) -> Result<StreamConnection> {
        let mut props = properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "DSP",
            *pw::keys::STREAM_CAPTURE_SINK => "true",
        };

        if let Some(target) = target {
            props.insert(*pw::keys::TARGET_OBJECT, target);
        }

        let stream = pw::stream::StreamRc::new(core.clone(), "oscavmgr-audio-reaction", props)
            .context("create PipeWire stream")?;

        let process_state = AudioProcessState {
            format: Default::default(),
            shared,
            smoothing: SmoothedReading::default(),
        };

        let listener = stream
            .add_local_listener_with_user_data(process_state)
            .state_changed(|_, user_data, _old, new| match new {
                pw::stream::StreamState::Streaming => {
                    set_status(&user_data.shared, AudioStatus::On);
                }
                pw::stream::StreamState::Error(_) => {
                    set_error(&user_data.shared);
                }
                _ => {
                    set_status(&user_data.shared, AudioStatus::Waiting);
                }
            })
            .param_changed(|_, user_data, id, param| {
                let Some(param) = param else {
                    return;
                };
                if id != pw::spa::param::ParamType::Format.as_raw() {
                    return;
                }

                let Ok((media_type, media_subtype)) = format_utils::parse_format(param) else {
                    return;
                };
                if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                    return;
                }

                if let Err(error) = user_data.format.parse(param) {
                    log::warn!("Failed to parse PipeWire audio format: {error:?}");
                }
            })
            .process(process_audio_buffer)
            .register()
            .context("register PipeWire stream listener")?;

        let values = format_param().context("build PipeWire F32LE format parameter")?;
        let mut params = [Pod::from_bytes(&values).context("parse PipeWire format parameter")?];

        stream
            .connect(
                spa::utils::Direction::Input,
                None,
                pw::stream::StreamFlags::AUTOCONNECT
                    | pw::stream::StreamFlags::MAP_BUFFERS
                    | pw::stream::StreamFlags::RT_PROCESS,
                &mut params,
            )
            .context("connect PipeWire stream")?;

        Ok(StreamConnection { listener, stream })
    }

    fn format_param() -> Result<Vec<u8>> {
        let mut audio_info = spa::param::audio::AudioInfoRaw::new();
        audio_info.set_format(spa::param::audio::AudioFormat::F32LE);

        let obj = spa::pod::Object {
            type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: spa::param::ParamType::EnumFormat.as_raw(),
            properties: audio_info.into(),
        };

        Ok(spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &spa::pod::Value::Object(obj),
        )?
        .0
        .into_inner())
    }

    fn process_audio_buffer(stream: &pw::stream::Stream, user_data: &mut AudioProcessState) {
        let Some(mut buffer) = stream.dequeue_buffer() else {
            return;
        };

        let datas = buffer.datas_mut();
        if datas.is_empty() {
            return;
        }

        let channels = user_data.format.channels() as usize;
        if channels == 0 {
            return;
        }

        let data = &mut datas[0];
        let chunk_sample_count = data.chunk().size() as usize / mem::size_of::<f32>();
        let Some(bytes) = data.data() else {
            return;
        };

        let sample_count = chunk_sample_count.min(bytes.len() / mem::size_of::<f32>());
        if sample_count == 0 {
            return;
        }

        let raw = analyze_f32le_bytes(bytes, sample_count, channels);
        let smoothed = user_data.smoothing.update(raw);
        set_reading(&user_data.shared, smoothed);
    }

    fn analyze_f32le_bytes(bytes: &[u8], sample_count: usize, channels: usize) -> AudioReading {
        let mut sums = super::AudioSums::new(channels);

        for (sample_index, chunk) in bytes
            .chunks_exact(mem::size_of::<f32>())
            .take(sample_count)
            .enumerate()
        {
            sums.add(
                sample_index,
                f32::from_le_bytes(chunk.try_into().expect("fixed chunk size")),
            );
        }

        sums.finish()
    }

    fn parse_sink_name(value: &str) -> Option<String> {
        serde_json::from_str::<Value>(value)
            .ok()
            .and_then(|value| match value {
                Value::String(name) => Some(name),
                Value::Object(object) => object
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ => None,
            })
            .or_else(|| {
                let trimmed = value.trim();
                let looks_like_json = trimmed.starts_with('{')
                    || trimmed.starts_with('[')
                    || trimmed.eq_ignore_ascii_case("null");
                (!trimmed.is_empty() && !looks_like_json).then(|| trimmed.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.0001,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn silence_gives_zero_volume_and_center_direction() {
        let reading = analyze_f32_samples(&[0.0, 0.0, 0.0, 0.0], 2);

        assert_close(reading.volume, 0.0);
        assert_close(reading.direction, 0.0);
    }

    #[test]
    fn left_only_gives_negative_direction() {
        let reading = analyze_f32_samples(&[1.0, 0.0, 1.0, 0.0], 2);

        assert_close(reading.direction, -1.0);
    }

    #[test]
    fn right_only_gives_positive_direction() {
        let reading = analyze_f32_samples(&[0.0, 1.0, 0.0, 1.0], 2);

        assert_close(reading.direction, 1.0);
    }

    #[test]
    fn balanced_stereo_gives_center_direction() {
        let reading = analyze_f32_samples(&[0.5, 0.5, -0.5, -0.5], 2);

        assert_close(reading.direction, 0.0);
    }

    #[test]
    fn mono_gives_center_direction() {
        let reading = analyze_f32_samples(&[1.0, 0.5, -0.5], 1);

        assert_close(reading.direction, 0.0);
        assert!(reading.volume > 0.0);
    }

    #[test]
    fn loud_samples_clamp_volume() {
        let reading = analyze_f32_samples(&[2.0, 2.0, 2.0, 2.0], 2);

        assert_close(reading.volume, 1.0);
    }
}
