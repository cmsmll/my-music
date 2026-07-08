import { convertFileSrc } from "@tauri-apps/api/core";
import type { PlaybackStatus, Track } from "../types/music";

export type FrontendAudioEvents = {
  status_change?: (status: PlaybackStatus) => void;
  ended?: (status: PlaybackStatus) => void;
  error?: (message: string) => void;
};

export class FrontendAudioPlayer {
  private readonly audio = new Audio();
  private path: string | null = null;
  private volume = 1;

  constructor(private readonly events: FrontendAudioEvents = {}) {
    this.audio.preload = "auto";
    this.audio.addEventListener("play", () => this.emit_status());
    this.audio.addEventListener("pause", () => this.emit_status());
    this.audio.addEventListener("timeupdate", () => this.emit_status());
    this.audio.addEventListener("ended", () => {
      this.emit_status();
      this.events.ended?.(this.status());
    });
    this.audio.addEventListener("error", () => {
      this.events.error?.(this.audio_error_message());
      this.emit_status();
    });
  }

  async play(track: Track, seconds = 0) {
    this.path = track.path;
    this.audio.src = convertFileSrc(track.path);
    this.audio.volume = this.safe_audio_volume(this.volume);
    if (seconds > 0) {
      await this.wait_for_metadata();
      this.audio.currentTime = Math.max(seconds, 0);
    }
    await this.audio.play();
    this.emit_status();
  }

  pause() {
    this.audio.pause();
    this.emit_status();
  }

  async resume() {
    if (!this.path) return;
    await this.audio.play();
    this.emit_status();
  }

  stop() {
    this.audio.pause();
    this.audio.removeAttribute("src");
    this.audio.load();
    this.path = null;
    this.emit_status();
  }

  seek(seconds: number) {
    this.audio.currentTime = Math.max(seconds, 0);
    this.emit_status();
  }

  set_volume(volume: number) {
    this.volume = Math.min(Math.max(volume, 0), 1.5);
    this.audio.volume = this.safe_audio_volume(this.volume);
    this.emit_status();
  }

  status(): PlaybackStatus {
    return {
      path: this.path,
      playing: !this.audio.paused && !this.audio.ended,
      volume: this.volume,
      elapsed: Math.max(Math.floor(this.audio.currentTime || 0), 0),
    };
  }

  destroy() {
    this.stop();
  }

  private emit_status() {
    this.events.status_change?.(this.status());
  }

  private safe_audio_volume(volume: number) {
    return Math.min(Math.max(volume, 0), 1);
  }

  private wait_for_metadata() {
    if (this.audio.readyState >= HTMLMediaElement.HAVE_METADATA) {
      return Promise.resolve();
    }

    return new Promise<void>((resolve, reject) => {
      const cleanup = () => {
        this.audio.removeEventListener("loadedmetadata", handle_loaded);
        this.audio.removeEventListener("error", handle_error);
      };
      const handle_loaded = () => {
        cleanup();
        resolve();
      };
      const handle_error = () => {
        cleanup();
        reject(new Error(this.audio_error_message()));
      };
      this.audio.addEventListener("loadedmetadata", handle_loaded, { once: true });
      this.audio.addEventListener("error", handle_error, { once: true });
      this.audio.load();
    });
  }

  private audio_error_message() {
    const error = this.audio.error;
    if (!error) return "前端音频播放失败";
    switch (error.code) {
      case MediaError.MEDIA_ERR_ABORTED:
        return "音频播放被中止";
      case MediaError.MEDIA_ERR_NETWORK:
        return "音频文件加载失败";
      case MediaError.MEDIA_ERR_DECODE:
        return "音频格式无法解码";
      case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
        return "当前音频格式不支持前端播放";
      default:
        return "前端音频播放失败";
    }
  }
}
