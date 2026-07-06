use crate::models::PlaybackStatus;
use crate::utils::unix_timestamp;
use rodio::{
    cpal::{
        self,
        traits::{DeviceTrait, HostTrait},
        StreamError,
    },
    source::{SineWave, Source},
    Decoder, DeviceSinkBuilder, MixerDeviceSink, Player,
};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub(crate) struct PlaybackSnapshot {
    path: Option<String>,
    playing: bool,
    volume: f32,
    elapsed_offset: Duration,
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_total: Duration,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            path: None,
            playing: false,
            volume: 1.0,
            elapsed_offset: Duration::ZERO,
            started_at: None,
            paused_at: None,
            paused_total: Duration::ZERO,
        }
    }
}

pub(crate) struct Playback {
    player: Player,
    started_at: Instant,
    last_player_pos: Duration,
    last_player_pos_changed_at: Instant,
    wake_attempts: u8,
    forced_restart_attempts: u8,
    next_forced_restart_at: Option<Instant>,
}

impl Playback {
    fn new(player: Player) -> Self {
        let now = Instant::now();
        Self {
            player,
            started_at: now,
            last_player_pos: Duration::ZERO,
            last_player_pos_changed_at: now,
            wake_attempts: 0,
            forced_restart_attempts: 0,
            next_forced_restart_at: Some(now + Duration::from_millis(1_800)),
        }
    }

    fn reset_watchdog(&mut self, pos: Duration) {
        let now = Instant::now();
        self.started_at = now;
        self.last_player_pos = pos;
        self.last_player_pos_changed_at = now;
        self.wake_attempts = 0;
        self.forced_restart_attempts = 0;
        self.next_forced_restart_at = Some(now + Duration::from_millis(1_800));
    }

    fn carry_forced_restart(&mut self, attempts: u8) {
        self.forced_restart_attempts = attempts;
        self.next_forced_restart_at = if attempts < MAX_FORCED_OUTPUT_RESTARTS {
            Some(Instant::now() + Duration::from_millis(2_400))
        } else {
            None
        };
    }
}

struct AudioOutput {
    sink: MixerDeviceSink,
    stream_errors: Receiver<String>,
    device_id: String,
    device_name: String,
}

struct PlaybackRebuild<'a> {
    path: &'a str,
    seconds: u64,
    volume: f32,
    playing: bool,
    stage: &'a str,
}

struct OutputDevice {
    device: cpal::Device,
    id: String,
    name: String,
}

const MAX_FORCED_OUTPUT_RESTARTS: u8 = 3;

pub(crate) struct AudioEngine {
    tx: Mutex<Option<Sender<AudioCommand>>>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    log_dir: PathBuf,
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
        let snapshot = Arc::new(Mutex::new(PlaybackSnapshot::default()));
        let log_dir = PathBuf::from(log_dir);

        Self {
            tx: Mutex::new(None),
            snapshot,
            log_dir,
        }
    }

    fn command_sender(&self) -> Result<Sender<AudioCommand>, String> {
        let mut tx = self.tx.lock().map_err(|_| {
            let reason = "audio engine sender is unavailable".to_string();
            write_audio_error_log(
                &self.log_dir,
                "音频命令失败",
                None,
                Some("lock_sender"),
                None,
                &reason,
            );
            reason
        })?;

        if tx.is_none() {
            *tx = Some(spawn_audio_worker(
                Arc::clone(&self.snapshot),
                self.log_dir.clone(),
            ));
        }

        tx.as_ref()
            .cloned()
            .ok_or_else(|| "音频线程初始化失败".to_string())
    }

    pub(crate) fn set_volume(&self, volume: f32) -> Result<(), String> {
        let next_volume = volume.clamp(0.0, 1.5);
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
                write_audio_error_log(
                    &self.log_dir,
                    "音频命令失败",
                    None,
                    Some("send_command"),
                    None,
                    &reason,
                );
                reason
            })?;
        reply_rx.recv().map_err(|_| {
            let reason = "音频线程没有返回结果".to_string();
            write_audio_error_log(
                &self.log_dir,
                "音频命令失败",
                None,
                Some("receive_command_result"),
                None,
                &reason,
            );
            reason
        })?
    }

    pub(crate) fn status(&self) -> Result<PlaybackStatus, String> {
        let snapshot = self.snapshot.lock().map_err(|_| {
            let reason = "audio engine state is unavailable".to_string();
            write_audio_error_log(
                &self.log_dir,
                "音频状态失败",
                None,
                Some("read_playback_status"),
                None,
                &reason,
            );
            reason
        })?;
        let snapshot = snapshot.clone();

        let elapsed = elapsed_seconds(&snapshot);

        Ok(PlaybackStatus {
            path: snapshot.path,
            playing: snapshot.playing,
            volume: snapshot.volume,
            elapsed,
        })
    }
}

fn spawn_audio_worker(
    thread_snapshot: Arc<Mutex<PlaybackSnapshot>>,
    thread_log_dir: PathBuf,
) -> Sender<AudioCommand> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut output: Option<AudioOutput> = None;
        let mut playback: Option<Playback> = None;

        loop {
            recover_from_stream_error(
                &mut output,
                &mut playback,
                &thread_snapshot,
                &thread_log_dir,
            );
            recover_from_default_device_change(
                &mut output,
                &mut playback,
                &thread_snapshot,
                &thread_log_dir,
            );
            recover_from_stalled_playback(
                &mut output,
                &mut playback,
                &thread_snapshot,
                &thread_log_dir,
            );
            recover_from_suspect_silent_playback(
                &mut output,
                &mut playback,
                &thread_snapshot,
                &thread_log_dir,
            );

            let command = match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            };

            match command {
                AudioCommand::Play { path, reply } => {
                    let result = (|| {
                        let file = File::open(&path).map_err(|err| {
                            let reason = format!("无法打开音频文件: {err}");
                            write_audio_error_log(
                                &thread_log_dir,
                                "音频播放失败",
                                Some(&path),
                                Some("open_file"),
                                None,
                                &reason,
                            );
                            reason
                        })?;
                        let source = Decoder::try_from(file).map_err(|err| {
                            let reason = format!("无法解码音频文件: {err}");
                            write_audio_error_log(
                                &thread_log_dir,
                                "音频播放失败",
                                Some(&path),
                                Some("decode_file"),
                                None,
                                &reason,
                            );
                            reason
                        })?;
                        let volume = thread_snapshot
                            .lock()
                            .map(|snapshot| snapshot.volume)
                            .unwrap_or(1.0);

                        let next_output =
                            open_audio_output(&thread_log_dir, Some(&path), "play_open_output")?;
                        prime_audio_output(&next_output);
                        let _ = wake_audio_engine(&thread_log_dir, Some(&path), "play_wake_output");
                        let player = Player::connect_new(next_output.sink.mixer());

                        player.append(source);
                        player.set_volume(volume);
                        player.play();

                        if let Some(current) = playback.take() {
                            current.player.stop();
                        }
                        output = Some(next_output);
                        playback = Some(Playback::new(player));

                        update_snapshot(&thread_snapshot, |state| {
                            state.path = Some(path);
                            state.playing = true;
                            state.volume = volume;
                            state.elapsed_offset = Duration::ZERO;
                            state.started_at = Some(Instant::now());
                            state.paused_at = None;
                            state.paused_total = Duration::ZERO;
                        })
                    })();

                    let _ = reply.send(result);
                }
                AudioCommand::Pause { reply } => {
                    if let Some(current) = playback.as_ref() {
                        current.player.pause();
                        let _ = update_snapshot(&thread_snapshot, |state| {
                            if state.paused_at.is_none() {
                                state.paused_at = Some(Instant::now());
                            }
                            state.playing = false;
                        });
                    }

                    let _ = reply.send(Ok(()));
                }
                AudioCommand::Resume { reply } => {
                    if let Some(current) = playback.as_ref() {
                        current.player.play();
                        let _ = update_snapshot(&thread_snapshot, |state| {
                            if let Some(paused_at) = state.paused_at.take() {
                                state.paused_total += paused_at.elapsed();
                            }
                            state.playing = true;
                        });
                    }

                    let _ = reply.send(Ok(()));
                }
                AudioCommand::Stop { reply } => {
                    if let Some(current) = playback.take() {
                        current.player.stop();
                    }
                    output = None;
                    let current_volume = thread_snapshot
                        .lock()
                        .map(|snapshot| snapshot.volume)
                        .unwrap_or(1.0);
                    let _ = update_snapshot(&thread_snapshot, |state| {
                        *state = PlaybackSnapshot {
                            volume: current_volume,
                            ..PlaybackSnapshot::default()
                        }
                    });
                    let _ = reply.send(Ok(()));
                }
                AudioCommand::SetVolume { volume, reply } => {
                    let next_volume = volume.clamp(0.0, 1.5);
                    if let Some(current) = playback.as_ref() {
                        current.player.set_volume(next_volume);
                    }
                    let _ = update_snapshot(&thread_snapshot, |state| state.volume = next_volume);
                    let _ = reply.send(Ok(()));
                }
                AudioCommand::Seek { seconds, reply } => {
                    let result = (|| {
                        let Some(current) = playback.as_mut() else {
                            return Ok(());
                        };

                        if let Err(err) = current.player.try_seek(Duration::from_secs(seconds)) {
                            let snapshot = thread_snapshot
                                .lock()
                                .map_err(|_| "audio engine state is unavailable".to_string())?
                                .clone();
                            let Some(path) = snapshot.path.clone() else {
                                return Ok(());
                            };
                            write_seek_error_log(
                                &thread_log_dir,
                                &path,
                                "current_player",
                                seconds,
                                &err.to_string(),
                            );

                            let file = File::open(&path).map_err(|err| {
                                let reason = format!("无法打开音频文件: {err}");
                                write_audio_error_log(
                                    &thread_log_dir,
                                    "音频跳转失败",
                                    Some(&path),
                                    Some("rebuilt_open_file"),
                                    Some(seconds),
                                    &reason,
                                );
                                reason
                            })?;
                            let source = Decoder::try_from(file).map_err(|err| {
                                let reason = format!("无法解码音频文件: {err}");
                                write_audio_error_log(
                                    &thread_log_dir,
                                    "音频跳转失败",
                                    Some(&path),
                                    Some("rebuilt_decode_file"),
                                    Some(seconds),
                                    &reason,
                                );
                                reason
                            })?;
                            let next_output = open_audio_output(
                                &thread_log_dir,
                                Some(&path),
                                "seek_rebuild_open_output",
                            )?;
                            prime_audio_output(&next_output);
                            let player = Player::connect_new(next_output.sink.mixer());

                            player.append(source);
                            player.set_volume(snapshot.volume);
                            if let Err(err) = player.try_seek(Duration::from_secs(seconds)) {
                                write_seek_error_log(
                                    &thread_log_dir,
                                    &path,
                                    "rebuilt_player",
                                    seconds,
                                    &err.to_string(),
                                );
                                return Err(format!("无法跳转播放进度: {err}"));
                            }
                            if snapshot.playing {
                                player.play();
                            } else {
                                player.pause();
                            }

                            if let Some(current) = playback.take() {
                                current.player.stop();
                            }
                            output = Some(next_output);
                            playback = Some(Playback::new(player));
                        } else {
                            current.reset_watchdog(Duration::from_secs(seconds));
                        }

                        update_snapshot(&thread_snapshot, |state| {
                            state.elapsed_offset = Duration::from_secs(seconds);
                            state.started_at = Some(Instant::now());
                            state.paused_total = Duration::ZERO;
                            state.paused_at = if state.playing {
                                None
                            } else {
                                Some(Instant::now())
                            };
                        })
                    })();

                    let _ = reply.send(result);
                }
            }
        }
    });

    tx
}

fn open_audio_output(
    log_dir: &Path,
    path: Option<&str>,
    stage: &str,
) -> Result<AudioOutput, String> {
    let default_device = default_output_device(log_dir, path, stage)?;

    let (stream_error_tx, stream_errors) = mpsc::channel();
    let mut sink = DeviceSinkBuilder::from_device(default_device.device)
        .map_err(|err| {
            let reason = format!("无法读取默认音频输出设备配置: {err}");
            write_audio_error_log(
                log_dir,
                "音频输出设备失败",
                path,
                Some(stage),
                None,
                &reason,
            );
            reason
        })?
        .with_error_callback(move |err| {
            if matches!(
                err,
                StreamError::DeviceNotAvailable | StreamError::StreamInvalidated
            ) {
                let _ = stream_error_tx.send(err.to_string());
            }
        })
        .open_sink_or_fallback()
        .map_err(|err| {
            let reason = format!("无法打开默认音频输出设备: {err}");
            write_audio_error_log(
                log_dir,
                "音频输出设备失败",
                path,
                Some(stage),
                None,
                &reason,
            );
            reason
        })?;
    sink.log_on_drop(false);
    Ok(AudioOutput {
        sink,
        stream_errors,
        device_id: default_device.id,
        device_name: default_device.name,
    })
}

fn prime_audio_output(output: &AudioOutput) {
    let warmup = SineWave::new(18_000.0)
        .amplify(0.001)
        .take_duration(Duration::from_millis(900));
    output.sink.mixer().add(warmup);
}

fn wake_audio_engine(log_dir: &Path, path: Option<&str>, stage: &str) -> Result<(), String> {
    let wake_output = open_audio_output(log_dir, path, stage)?;
    let wakeup = SineWave::new(18_000.0)
        .amplify(0.001)
        .take_duration(Duration::from_millis(900));
    wake_output.sink.mixer().add(wakeup);

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(1_000));
        drop(wake_output);
    });

    Ok(())
}

fn default_output_device(
    log_dir: &Path,
    path: Option<&str>,
    stage: &str,
) -> Result<OutputDevice, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| {
            let reason = "找不到默认音频输出设备".to_string();
            write_audio_error_log(
                log_dir,
                "音频输出设备失败",
                path,
                Some(stage),
                None,
                &reason,
            );
            reason
        })?;

    let id = device.id().map_err(|err| {
        let reason = format!("无法读取默认音频输出设备ID: {err}");
        write_audio_error_log(
            log_dir,
            "音频输出设备失败",
            path,
            Some(stage),
            None,
            &reason,
        );
        reason
    })?;
    let name = device
        .description()
        .map(|description| description.name().to_string())
        .unwrap_or_else(|_| "未知音频设备".to_string());

    Ok(OutputDevice {
        device,
        id: id.to_string(),
        name,
    })
}

fn current_default_output_identity() -> Option<(String, String)> {
    let device = cpal::default_host().default_output_device()?;
    let id = device.id().ok()?.to_string();
    let name = device
        .description()
        .map(|description| description.name().to_string())
        .unwrap_or_else(|_| "未知音频设备".to_string());
    Some((id, name))
}

fn recover_from_stream_error(
    output: &mut Option<AudioOutput>,
    playback: &mut Option<Playback>,
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    log_dir: &Path,
) {
    let Some(reason) = drain_stream_error(output.as_ref()) else {
        return;
    };

    let Ok(current_snapshot) = snapshot.lock().map(|snapshot| snapshot.clone()) else {
        write_audio_error_log(
            log_dir,
            "音频输出流恢复失败",
            None,
            Some("read_recovery_snapshot"),
            None,
            "audio engine state is unavailable",
        );
        return;
    };
    let Some(path) = current_snapshot.path.clone() else {
        write_audio_error_log(
            log_dir,
            "音频输出流失效",
            None,
            Some("stream_error_callback"),
            None,
            &reason,
        );
        return;
    };

    let seconds = elapsed_seconds(&current_snapshot);
    write_audio_error_log(
        log_dir,
        "音频输出流失效",
        Some(&path),
        Some("stream_error_callback"),
        Some(seconds),
        &reason,
    );

    let rebuild = PlaybackRebuild {
        path: &path,
        seconds,
        volume: current_snapshot.volume,
        playing: current_snapshot.playing,
        stage: "stream_error_recover",
    };
    if let Err(err) = rebuild_playback_at(output, playback, log_dir, rebuild) {
        write_audio_error_log(
            log_dir,
            "音频输出流恢复失败",
            Some(&path),
            Some("stream_error_recover"),
            Some(seconds),
            &err,
        );
        return;
    }

    let _ = update_snapshot(snapshot, |state| {
        state.elapsed_offset = Duration::from_secs(seconds);
        state.started_at = Some(Instant::now());
        state.paused_total = Duration::ZERO;
        state.paused_at = if state.playing {
            None
        } else {
            Some(Instant::now())
        };
    });
}

fn recover_from_default_device_change(
    output: &mut Option<AudioOutput>,
    playback: &mut Option<Playback>,
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    log_dir: &Path,
) {
    let Some(current_output) = output.as_ref() else {
        return;
    };
    let Some((default_device_id, default_device_name)) = current_default_output_identity() else {
        return;
    };
    if default_device_id == current_output.device_id {
        return;
    }

    let Ok(current_snapshot) = snapshot.lock().map(|snapshot| snapshot.clone()) else {
        write_audio_error_log(
            log_dir,
            "默认音频输出设备切换失败",
            None,
            Some("read_device_change_snapshot"),
            None,
            "audio engine state is unavailable",
        );
        return;
    };
    let Some(path) = current_snapshot.path.clone() else {
        return;
    };

    let seconds = elapsed_seconds(&current_snapshot);
    let reason = format!(
        "默认输出设备从 {}({}) 切换到 {}({})",
        current_output.device_name,
        current_output.device_id,
        default_device_name,
        default_device_id
    );
    write_audio_error_log(
        log_dir,
        "默认音频输出设备变化",
        Some(&path),
        Some("default_device_changed"),
        Some(seconds),
        &reason,
    );

    let rebuild = PlaybackRebuild {
        path: &path,
        seconds,
        volume: current_snapshot.volume,
        playing: current_snapshot.playing,
        stage: "default_device_changed",
    };
    if let Err(err) = rebuild_playback_at(output, playback, log_dir, rebuild) {
        write_audio_error_log(
            log_dir,
            "默认音频输出设备切换失败",
            Some(&path),
            Some("default_device_changed"),
            Some(seconds),
            &err,
        );
        return;
    }

    let _ = update_snapshot(snapshot, |state| {
        state.elapsed_offset = Duration::from_secs(seconds);
        state.started_at = Some(Instant::now());
        state.paused_total = Duration::ZERO;
        state.paused_at = if state.playing {
            None
        } else {
            Some(Instant::now())
        };
    });
}

fn recover_from_stalled_playback(
    output: &mut Option<AudioOutput>,
    playback: &mut Option<Playback>,
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    log_dir: &Path,
) {
    let Some((path, seconds, volume, playing, reason)) =
        stalled_playback_recovery_request(playback, snapshot, log_dir)
    else {
        return;
    };

    write_audio_error_log(
        log_dir,
        "音频播放自检停滞",
        Some(&path),
        Some("player_position_stalled"),
        Some(seconds),
        &reason,
    );

    let _ = wake_audio_engine(log_dir, Some(&path), "stalled_playback_wakeup");
    let rebuild = PlaybackRebuild {
        path: &path,
        seconds,
        volume,
        playing,
        stage: "stalled_playback_rebuild",
    };
    if let Err(err) = rebuild_playback_at(output, playback, log_dir, rebuild) {
        write_audio_error_log(
            log_dir,
            "音频播放自检恢复失败",
            Some(&path),
            Some("stalled_playback_rebuild"),
            Some(seconds),
            &err,
        );
        if let Some(current) = playback.as_mut() {
            current.last_player_pos_changed_at = Instant::now();
        }
        return;
    }

    let _ = update_snapshot(snapshot, |state| {
        state.elapsed_offset = Duration::from_secs(seconds);
        state.started_at = Some(Instant::now());
        state.paused_total = Duration::ZERO;
        state.paused_at = if state.playing {
            None
        } else {
            Some(Instant::now())
        };
    });
}

fn stalled_playback_recovery_request(
    playback: &mut Option<Playback>,
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    log_dir: &Path,
) -> Option<(String, u64, f32, bool, String)> {
    let current = playback.as_mut()?;
    if current.player.empty() || current.started_at.elapsed() < Duration::from_millis(1_500) {
        return None;
    }

    let current_snapshot = match snapshot.lock().map(|snapshot| snapshot.clone()) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            write_audio_error_log(
                log_dir,
                "音频播放自检失败",
                None,
                Some("read_stalled_snapshot"),
                None,
                "audio engine state is unavailable",
            );
            return None;
        }
    };
    if !current_snapshot.playing {
        return None;
    }

    let player_pos = current.player.get_pos();
    if duration_changed(
        player_pos,
        current.last_player_pos,
        Duration::from_millis(50),
    ) {
        current.last_player_pos = player_pos;
        current.last_player_pos_changed_at = Instant::now();
        current.wake_attempts = 0;
        return None;
    }
    if current.last_player_pos_changed_at.elapsed() < Duration::from_millis(1_500) {
        return None;
    }

    current.wake_attempts = current.wake_attempts.saturating_add(1);
    current.last_player_pos_changed_at = Instant::now();

    let path = current_snapshot.path.clone()?;
    let seconds = elapsed_seconds(&current_snapshot);
    let reason = format!(
        "rodio位置停在{}ms，状态进度{}s，尝试恢复第{}次",
        player_pos.as_millis(),
        seconds,
        current.wake_attempts
    );
    Some((
        path,
        seconds,
        current_snapshot.volume,
        current_snapshot.playing,
        reason,
    ))
}

fn recover_from_suspect_silent_playback(
    output: &mut Option<AudioOutput>,
    playback: &mut Option<Playback>,
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    log_dir: &Path,
) {
    let Some((path, seconds, volume, playing, next_attempt, player_pos)) =
        forced_output_restart_request(playback, snapshot, log_dir)
    else {
        return;
    };

    let reason = format!(
        "主动重启输出流第{}次，rodio位置{}ms，状态进度{}s",
        next_attempt,
        player_pos.as_millis(),
        seconds
    );
    write_audio_error_log(
        log_dir,
        "音频播放健康检查重启",
        Some(&path),
        Some("forced_output_restart"),
        Some(seconds),
        &reason,
    );

    let _ = wake_audio_engine(log_dir, Some(&path), "forced_output_restart_wakeup");
    let rebuild = PlaybackRebuild {
        path: &path,
        seconds,
        volume,
        playing,
        stage: "forced_output_restart",
    };
    if let Err(err) = rebuild_playback_at(output, playback, log_dir, rebuild) {
        write_audio_error_log(
            log_dir,
            "音频播放健康检查重启失败",
            Some(&path),
            Some("forced_output_restart"),
            Some(seconds),
            &err,
        );
        if let Some(current) = playback.as_mut() {
            current.carry_forced_restart(next_attempt);
        }
        return;
    }

    if let Some(current) = playback.as_mut() {
        current.carry_forced_restart(next_attempt);
    }
    let _ = update_snapshot(snapshot, |state| {
        state.elapsed_offset = Duration::from_secs(seconds);
        state.started_at = Some(Instant::now());
        state.paused_total = Duration::ZERO;
        state.paused_at = if state.playing {
            None
        } else {
            Some(Instant::now())
        };
    });
}

fn forced_output_restart_request(
    playback: &mut Option<Playback>,
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    log_dir: &Path,
) -> Option<(String, u64, f32, bool, u8, Duration)> {
    let current = playback.as_mut()?;
    let next_restart_at = current.next_forced_restart_at?;
    if current.player.empty()
        || current.forced_restart_attempts >= MAX_FORCED_OUTPUT_RESTARTS
        || Instant::now() < next_restart_at
    {
        return None;
    }

    let current_snapshot = match snapshot.lock().map(|snapshot| snapshot.clone()) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            write_audio_error_log(
                log_dir,
                "音频播放健康检查失败",
                None,
                Some("read_forced_restart_snapshot"),
                None,
                "audio engine state is unavailable",
            );
            return None;
        }
    };
    if !current_snapshot.playing {
        current.next_forced_restart_at = Some(Instant::now() + Duration::from_millis(1_000));
        return None;
    }

    let path = current_snapshot.path.clone()?;
    let player_pos = current.player.get_pos();
    let seconds = elapsed_seconds(&current_snapshot).max(player_pos.as_secs());
    let next_attempt = current.forced_restart_attempts.saturating_add(1);
    current.forced_restart_attempts = next_attempt;
    current.next_forced_restart_at = None;
    Some((
        path,
        seconds,
        current_snapshot.volume,
        current_snapshot.playing,
        next_attempt,
        player_pos,
    ))
}

fn duration_changed(current: Duration, previous: Duration, threshold: Duration) -> bool {
    current
        .checked_sub(previous)
        .or_else(|| previous.checked_sub(current))
        .is_some_and(|delta| delta >= threshold)
}

fn drain_stream_error(output: Option<&AudioOutput>) -> Option<String> {
    let output = output?;
    let mut latest_error = None;

    loop {
        match output.stream_errors.try_recv() {
            Ok(err) => latest_error = Some(err),
            Err(TryRecvError::Empty) => return latest_error,
            Err(TryRecvError::Disconnected) => {
                return latest_error.or_else(|| Some("音频输出错误回调已断开".to_string()));
            }
        }
    }
}

fn rebuild_playback_at(
    output: &mut Option<AudioOutput>,
    playback: &mut Option<Playback>,
    log_dir: &Path,
    request: PlaybackRebuild<'_>,
) -> Result<(), String> {
    let path = request.path;
    let seconds = request.seconds;
    let file = File::open(path).map_err(|err| {
        let reason = format!("无法打开音频文件: {err}");
        write_audio_error_log(
            log_dir,
            "音频播放恢复失败",
            Some(path),
            Some("recover_open_file"),
            Some(seconds),
            &reason,
        );
        reason
    })?;
    let source = Decoder::try_from(file).map_err(|err| {
        let reason = format!("无法解码音频文件: {err}");
        write_audio_error_log(
            log_dir,
            "音频播放恢复失败",
            Some(path),
            Some("recover_decode_file"),
            Some(seconds),
            &reason,
        );
        reason
    })?;
    let next_output = open_audio_output(log_dir, Some(path), request.stage)?;
    prime_audio_output(&next_output);
    let player = Player::connect_new(next_output.sink.mixer());

    player.append(source);
    player.set_volume(request.volume);
    if seconds > 0 {
        player
            .try_seek(Duration::from_secs(seconds))
            .map_err(|err| {
                write_seek_error_log(log_dir, path, "recover_player", seconds, &err.to_string());
                format!("无法恢复播放进度: {err}")
            })?;
    }
    if request.playing {
        player.play();
    } else {
        player.pause();
    }

    if let Some(current) = playback.take() {
        current.player.stop();
    }
    *output = Some(next_output);
    *playback = Some(Playback::new(player));
    Ok(())
}

pub(crate) fn update_snapshot(
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    change: impl FnOnce(&mut PlaybackSnapshot),
) -> Result<(), String> {
    let mut state = snapshot
        .lock()
        .map_err(|_| "audio engine state is unavailable".to_string())?;
    change(&mut state);
    Ok(())
}

pub(crate) fn elapsed_seconds(snapshot: &PlaybackSnapshot) -> u64 {
    let Some(started_at) = snapshot.started_at else {
        return 0;
    };

    let active_elapsed = match snapshot.paused_at {
        Some(paused_at) => paused_at.duration_since(started_at),
        None => started_at.elapsed(),
    };

    (snapshot.elapsed_offset + active_elapsed.saturating_sub(snapshot.paused_total)).as_secs()
}

fn write_seek_error_log(log_dir: &Path, path: &str, stage: &str, seconds: u64, reason: &str) {
    write_audio_error_log(
        log_dir,
        "音频跳转失败",
        Some(path),
        Some(stage),
        Some(seconds),
        reason,
    );
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
