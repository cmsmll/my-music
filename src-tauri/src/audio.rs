use crate::models::PlaybackStatus;
use rodio::{Decoder, DeviceSinkBuilder, Player};
use std::{
    fs::File,
    sync::{
        mpsc::{self, Sender},
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
}

pub(crate) struct AudioEngine {
    tx: Sender<AudioCommand>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
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
    pub(crate) fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(PlaybackSnapshot::default()));
        let thread_snapshot = Arc::clone(&snapshot);

        thread::spawn(move || {
            let stream_handle = DeviceSinkBuilder::open_default_sink();
            let Ok(stream_handle) = stream_handle else {
                while let Ok(command) = rx.recv() {
                    respond(command, Err("无法打开默认音频输出设备".to_string()));
                }
                return;
            };

            let mut playback: Option<Playback> = None;

            while let Ok(command) = rx.recv() {
                match command {
                    AudioCommand::Play { path, reply } => {
                        let result = (|| {
                            let file = File::open(&path)
                                .map_err(|err| format!("无法打开音频文件: {err}"))?;
                            let source = Decoder::try_from(file)
                                .map_err(|err| format!("无法解码音频文件: {err}"))?;
                            let player = Player::connect_new(stream_handle.mixer());

                            if let Some(current) = playback.take() {
                                current.player.stop();
                            }

                            player.append(source);
                            player.set_volume(1.0);
                            player.play();
                            playback = Some(Playback { player });

                            update_snapshot(&thread_snapshot, |state| {
                                state.path = Some(path);
                                state.playing = true;
                                state.volume = 1.0;
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
                        let _ = update_snapshot(&thread_snapshot, |state| {
                            *state = PlaybackSnapshot::default()
                        });
                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::SetVolume { volume, reply } => {
                        let next_volume = volume.clamp(0.0, 1.5);
                        if let Some(current) = playback.as_ref() {
                            current.player.set_volume(next_volume);
                        }
                        let _ =
                            update_snapshot(&thread_snapshot, |state| state.volume = next_volume);
                        let _ = reply.send(Ok(()));
                    }
                    AudioCommand::Seek { seconds, reply } => {
                        let result = (|| {
                            let Some(current) = playback.as_ref() else {
                                return Ok(());
                            };

                            if let Err(err) = &current.player.try_seek(Duration::from_secs(seconds))
                            {
                                eprintln!("音频跳转失败:{err}");
                                let snapshot = thread_snapshot
                                    .lock()
                                    .map_err(|_| "audio engine state is unavailable".to_string())?
                                    .clone();
                                let Some(path) = snapshot.path.clone() else {
                                    return Ok(());
                                };

                                let file = File::open(&path)
                                    .map_err(|err| format!("无法打开音频文件: {err}"))?;
                                let source = Decoder::try_from(file)
                                    .map_err(|err| format!("无法解码音频文件: {err}"))?;
                                let player = Player::connect_new(stream_handle.mixer());

                                player.append(source);
                                player.set_volume(snapshot.volume);
                                player
                                    .try_seek(Duration::from_secs(seconds))
                                    .map_err(|err| format!("无法跳转播放进度: {err}"))?;
                                if snapshot.playing {
                                    player.play();
                                } else {
                                    player.pause();
                                }

                                if let Some(current) = playback.take() {
                                    current.player.stop();
                                }
                                playback = Some(Playback { player });
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

        Self { tx, snapshot }
    }

    pub(crate) fn send(
        &self,
        command: impl FnOnce(Sender<Result<(), String>>) -> AudioCommand,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(command(reply_tx))
            .map_err(|_| "音频线程已停止".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "音频线程没有返回结果".to_string())?
    }

    pub(crate) fn status(&self) -> Result<PlaybackStatus, String> {
        let snapshot = self
            .snapshot
            .lock()
            .map_err(|_| "audio engine state is unavailable".to_string())?
            .clone();

        let elapsed = elapsed_seconds(&snapshot);

        Ok(PlaybackStatus {
            path: snapshot.path,
            playing: snapshot.playing,
            volume: snapshot.volume,
            elapsed,
        })
    }
}

pub(crate) fn respond(command: AudioCommand, result: Result<(), String>) {
    match command {
        AudioCommand::Play { reply, .. }
        | AudioCommand::Pause { reply }
        | AudioCommand::Resume { reply }
        | AudioCommand::Stop { reply }
        | AudioCommand::SetVolume { reply, .. }
        | AudioCommand::Seek { reply, .. } => {
            let _ = reply.send(result);
        }
    }
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
