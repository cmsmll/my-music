use crate::models::PlaybackStatus;
use crate::utils::unix_timestamp;
use rodio::{
    cpal::{
        self,
        traits::{DeviceTrait, HostTrait},
    },
    Decoder, DeviceSinkBuilder, MixerDeviceSink, Player,
};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const WORKER_TICK_INTERVAL: Duration = Duration::from_millis(1000);
const MAX_VOLUME: f32 = 1.5;

#[derive(Debug, Clone)]
struct PlaybackSnapshot {
    path: Option<String>,
    playing: bool,
    volume: f32,
    elapsed_offset: Duration,
    started_at: Option<Instant>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            path: None,
            playing: false,
            volume: 1.0,
            elapsed_offset: Duration::ZERO,
            started_at: None,
        }
    }
}

impl PlaybackSnapshot {
    fn status(&self) -> PlaybackStatus {
        PlaybackStatus {
            path: self.path.clone(),
            playing: self.playing,
            volume: self.volume,
            elapsed: self.elapsed_seconds(),
        }
    }

    fn elapsed_seconds(&self) -> u64 {
        if !self.playing {
            return self.elapsed_offset.as_secs();
        }

        let Some(started_at) = self.started_at else {
            return self.elapsed_offset.as_secs();
        };

        (self.elapsed_offset + started_at.elapsed()).as_secs()
    }
}

struct Playback {
    path: String,
    _stream: MixerDeviceSink,
    player: Player,
    device: Option<OutputIdentity>,
}

impl Playback {
    fn open(
        path: String,
        position: Duration,
        volume: f32,
        playing: bool,
        logger: &AudioLogger,
        event: &str,
        stage: &str,
    ) -> Result<Self, String> {
        let source = open_decoder(&path, logger, event)?;
        let mut stream = open_default_sink(logger, Some(&path), stage)?;
        stream.log_on_drop(false);

        let player = Player::connect_new(stream.mixer());
        player.append(source);
        player.set_volume(volume);
        if position > Duration::ZERO {
            player.try_seek(position).map_err(|err| {
                let reason = format!("无法跳转播放进度: {err}");
                logger.write(
                    event,
                    Some(&path),
                    Some(stage),
                    Some(position.as_secs()),
                    &reason,
                );
                reason
            })?;
        }

        if playing {
            player.play();
        } else {
            player.pause();
        }

        Ok(Self {
            path,
            _stream: stream,
            player,
            device: OutputIdentity::current(),
        })
    }

    fn pause(&self) {
        self.player.pause();
    }

    fn resume(&self) {
        self.player.play();
    }

    fn stop(&self) {
        self.player.stop();
    }

    fn set_volume(&self, volume: f32) {
        self.player.set_volume(volume);
    }

    fn seek(&self, position: Duration) -> Result<Duration, String> {
        self.player
            .try_seek(position)
            .map_err(|err| err.to_string())?;
        Ok(self.position())
    }

    fn position(&self) -> Duration {
        self.player.get_pos()
    }

    fn is_finished(&self) -> bool {
        self.player.empty()
    }
}

#[derive(Clone)]
struct OutputIdentity {
    id: String,
    name: String,
}

impl OutputIdentity {
    fn current() -> Option<Self> {
        let device = cpal::default_host().default_output_device()?;
        let id = device.id().ok()?.to_string();
        let name = device
            .description()
            .map(|description| description.name().to_string())
            .unwrap_or_else(|_| "未知音频设备".to_string());
        Some(Self { id, name })
    }
}

#[derive(Clone)]
struct AudioLogger {
    dir: PathBuf,
}

impl AudioLogger {
    fn new(dir: String) -> Self {
        Self {
            dir: PathBuf::from(dir),
        }
    }

    fn write(
        &self,
        event: &str,
        path: Option<&str>,
        stage: Option<&str>,
        target_seconds: Option<u64>,
        reason: &str,
    ) {
        write_audio_error_log(&self.dir, event, path, stage, target_seconds, reason);
    }

    fn command_error(&self, stage: &str, reason: &str) {
        self.write("音频命令失败", None, Some(stage), None, reason);
    }

    fn seek_error(&self, path: &str, stage: &str, seconds: u64, reason: &str) {
        self.write(
            "音频跳转失败",
            Some(path),
            Some(stage),
            Some(seconds),
            reason,
        );
    }
}

pub(crate) struct AudioEngine {
    tx: Mutex<Option<Sender<AudioCommand>>>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    logger: AudioLogger,
}

pub(crate) enum AudioCommand {
    Play {
        path: String,
        reply: Sender<Result<(), String>>,
    },
    Pause {
        reply: Sender<Result<(), String>>,
    },
    Resume {
        reply: Sender<Result<(), String>>,
    },
    Stop {
        reply: Sender<Result<(), String>>,
    },
    SetVolume {
        volume: f32,
        reply: Sender<Result<(), String>>,
    },
    Seek {
        seconds: u64,
        reply: Sender<Result<(), String>>,
    },
}

impl AudioEngine {
    pub(crate) fn new(log_dir: String) -> Self {
        Self {
            tx: Mutex::new(None),
            snapshot: Arc::new(Mutex::new(PlaybackSnapshot::default())),
            logger: AudioLogger::new(log_dir),
        }
    }

    fn command_sender(&self) -> Result<Sender<AudioCommand>, String> {
        let mut tx = self.tx.lock().map_err(|_| {
            let reason = "audio engine sender is unavailable".to_string();
            self.logger.command_error("lock_sender", &reason);
            reason
        })?;

        if tx.is_none() {
            *tx = Some(AudioWorker::spawn(
                Arc::clone(&self.snapshot),
                self.logger.clone(),
            ));
        }

        tx.as_ref()
            .cloned()
            .ok_or_else(|| "音频线程初始化失败".to_string())
    }

    pub(crate) fn set_volume(&self, volume: f32) -> Result<(), String> {
        let next_volume = clamp_volume(volume);
        update_snapshot(&self.snapshot, |state| state.volume = next_volume)?;

        let Some(tx) = self.tx.lock().ok().and_then(|tx| tx.as_ref().cloned()) else {
            return Ok(());
        };

        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(AudioCommand::SetVolume {
            volume: next_volume,
            reply: reply_tx,
        })
        .map_err(|_| "音频线程已停止".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "音频线程没有返回结果".to_string())?
    }

    pub(crate) fn send(
        &self,
        command: impl FnOnce(Sender<Result<(), String>>) -> AudioCommand,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.command_sender()?
            .send(command(reply_tx))
            .map_err(|_| {
                let reason = "音频线程已停止".to_string();
                self.logger.command_error("send_command", &reason);
                reason
            })?;

        reply_rx.recv().map_err(|_| {
            let reason = "音频线程没有返回结果".to_string();
            self.logger.command_error("receive_command_result", &reason);
            reason
        })?
    }

    pub(crate) fn status(&self) -> Result<PlaybackStatus, String> {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.status())
            .map_err(|_| {
                let reason = "audio engine state is unavailable".to_string();
                self.logger.write(
                    "音频状态失败",
                    None,
                    Some("read_playback_status"),
                    None,
                    &reason,
                );
                reason
            })
    }
}

struct AudioWorker {
    playback: Option<Playback>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    logger: AudioLogger,
}

impl AudioWorker {
    fn spawn(snapshot: Arc<Mutex<PlaybackSnapshot>>, logger: AudioLogger) -> Sender<AudioCommand> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            Self {
                playback: None,
                snapshot,
                logger,
            }
            .run(rx);
        });

        tx
    }

    fn run(&mut self, rx: mpsc::Receiver<AudioCommand>) {
        loop {
            self.tick();

            let command = match rx.recv_timeout(WORKER_TICK_INTERVAL) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };

            self.handle_command(command);
        }
    }

    fn tick(&mut self) {
        self.recover_from_default_device_change();
        self.sync_snapshot_from_playback();
    }

    fn handle_command(&mut self, command: AudioCommand) {
        match command {
            AudioCommand::Play { path, reply } => {
                let _ = reply.send(self.play(path));
            }
            AudioCommand::Pause { reply } => {
                self.pause();
                let _ = reply.send(Ok(()));
            }
            AudioCommand::Resume { reply } => {
                self.resume();
                let _ = reply.send(Ok(()));
            }
            AudioCommand::Stop { reply } => {
                self.stop();
                let _ = reply.send(Ok(()));
            }
            AudioCommand::SetVolume { volume, reply } => {
                self.set_volume(volume);
                let _ = reply.send(Ok(()));
            }
            AudioCommand::Seek { seconds, reply } => {
                let _ = reply.send(self.seek(seconds));
            }
        }
    }

    fn play(&mut self, path: String) -> Result<(), String> {
        let volume = self.snapshot().volume;
        let playback = Playback::open(
            path.clone(),
            Duration::ZERO,
            volume,
            true,
            &self.logger,
            "音频播放失败",
            "play_open_default_sink",
        )?;

        self.replace_playback(playback);
        self.set_snapshot(path, Duration::ZERO, true, volume);
        Ok(())
    }

    fn pause(&self) {
        if let Some(playback) = self.playback.as_ref() {
            playback.pause();
            self.set_position(playback.position(), false);
        }
    }

    fn resume(&self) {
        if let Some(playback) = self.playback.as_ref() {
            playback.resume();
            self.set_position(playback.position(), true);
        }
    }

    fn stop(&mut self) {
        if let Some(playback) = self.playback.take() {
            playback.stop();
        }

        let volume = self.snapshot().volume;
        let _ = update_snapshot(&self.snapshot, |state| {
            *state = PlaybackSnapshot {
                volume,
                ..PlaybackSnapshot::default()
            };
        });
    }

    fn set_volume(&self, volume: f32) {
        let next_volume = clamp_volume(volume);
        if let Some(playback) = self.playback.as_ref() {
            playback.set_volume(next_volume);
        }
        let _ = update_snapshot(&self.snapshot, |state| state.volume = next_volume);
    }

    fn seek(&mut self, seconds: u64) -> Result<(), String> {
        let Some(playback) = self.playback.as_ref() else {
            return Ok(());
        };

        let target = Duration::from_secs(seconds);
        match playback.seek(target) {
            Ok(position) => {
                self.set_position(position, self.snapshot().playing);
                Ok(())
            }
            Err(err) => {
                let path = playback.path.clone();
                self.logger
                    .seek_error(&path, "current_player", seconds, &err);
                self.rebuild_playback(path, target, "音频跳转失败", "seek_rebuild")
            }
        }
    }

    fn recover_from_default_device_change(&mut self) {
        let Some(playback) = self.playback.as_ref() else {
            return;
        };
        if playback.is_finished() {
            return;
        }

        let Some(current_device) = playback.device.as_ref() else {
            return;
        };
        let Some(next_device) = OutputIdentity::current() else {
            return;
        };
        if next_device.id == current_device.id {
            return;
        }

        let path = playback.path.clone();
        let position = playback.position();
        let reason = format!(
            "默认输出设备从 {}({}) 切换到 {}({})",
            current_device.name, current_device.id, next_device.name, next_device.id
        );
        self.logger.write(
            "默认音频输出设备变化",
            Some(&path),
            Some("default_device_changed"),
            Some(position.as_secs()),
            &reason,
        );

        if let Err(err) = self.rebuild_playback(
            path.clone(),
            position,
            "默认音频输出设备切换失败",
            "default_device_changed",
        ) {
            self.logger.write(
                "默认音频输出设备切换失败",
                Some(&path),
                Some("default_device_changed"),
                Some(position.as_secs()),
                &err,
            );
        }
    }

    fn rebuild_playback(
        &mut self,
        path: String,
        position: Duration,
        event: &str,
        stage: &str,
    ) -> Result<(), String> {
        let snapshot = self.snapshot();
        let playback = Playback::open(
            path.clone(),
            position,
            snapshot.volume,
            snapshot.playing,
            &self.logger,
            event,
            stage,
        )?;

        self.replace_playback(playback);
        self.set_snapshot(path, position, snapshot.playing, snapshot.volume);
        Ok(())
    }

    fn replace_playback(&mut self, playback: Playback) {
        if let Some(current) = self.playback.replace(playback) {
            current.stop();
        }
    }

    fn sync_snapshot_from_playback(&self) {
        let Some(playback) = self.playback.as_ref() else {
            return;
        };

        let position = playback.position();
        let finished = playback.is_finished();
        let _ = update_snapshot(&self.snapshot, |state| {
            if state.path.as_deref() != Some(playback.path.as_str()) {
                return;
            }
            state.elapsed_offset = position;
            state.started_at = Some(Instant::now());
            if state.playing && finished {
                state.playing = false;
            }
        });
    }

    fn set_snapshot(&self, path: String, position: Duration, playing: bool, volume: f32) {
        let _ = update_snapshot(&self.snapshot, |state| {
            state.path = Some(path);
            state.playing = playing;
            state.volume = volume;
            state.elapsed_offset = position;
            state.started_at = Some(Instant::now());
        });
    }

    fn set_position(&self, position: Duration, playing: bool) {
        let _ = update_snapshot(&self.snapshot, |state| {
            state.playing = playing;
            state.elapsed_offset = position;
            state.started_at = Some(Instant::now());
        });
    }

    fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default()
    }
}

fn clamp_volume(volume: f32) -> f32 {
    volume.clamp(0.0, MAX_VOLUME)
}

fn open_decoder(
    path: &str,
    logger: &AudioLogger,
    event: &str,
) -> Result<Decoder<std::io::BufReader<File>>, String> {
    let file = File::open(path).map_err(|err| {
        let reason = format!("无法打开音频文件: {err}");
        logger.write(event, Some(path), Some("open_file"), None, &reason);
        reason
    })?;

    Decoder::try_from(file).map_err(|err| {
        let reason = format!("无法解码音频文件: {err}");
        logger.write(event, Some(path), Some("decode_file"), None, &reason);
        reason
    })
}

fn open_default_sink(
    logger: &AudioLogger,
    path: Option<&str>,
    stage: &str,
) -> Result<MixerDeviceSink, String> {
    DeviceSinkBuilder::open_default_sink().map_err(|err| {
        let reason = format!("无法打开默认音频输出设备: {err}");
        logger.write("音频输出设备失败", path, Some(stage), None, &reason);
        reason
    })
}

fn update_snapshot(
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    change: impl FnOnce(&mut PlaybackSnapshot),
) -> Result<(), String> {
    let mut state = snapshot
        .lock()
        .map_err(|_| "audio engine state is unavailable".to_string())?;
    change(&mut state);
    Ok(())
}

fn write_audio_error_log(
    log_dir: &Path,
    event: &str,
    path: Option<&str>,
    stage: Option<&str>,
    target_seconds: Option<u64>,
    reason: &str,
) {
    let _ = std::fs::create_dir_all(log_dir);
    let log_path = log_dir.join("audio.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return;
    };

    let song = path.map(log_value).unwrap_or_else(|| "无".to_string());
    let stage = stage.unwrap_or("unknown");
    let target_seconds = target_seconds
        .map(|seconds| seconds.to_string())
        .unwrap_or_else(|| "无".to_string());

    let _ = writeln!(
        file,
        "[{}] {} | 歌曲=\"{}\" | 阶段={} | 目标秒数={} | 原因=\"{}\"",
        unix_timestamp(),
        event,
        song,
        stage,
        target_seconds,
        log_value(reason)
    );
}

fn log_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
