import { convertFileSrc } from "@tauri-apps/api/core";
import type { PlaybackStatus, Track } from "../types/music";

export type FrontendAudioError = {
  path: string | null;
  source: string;
  code: number | null;
  message: string;
  elapsed: number;
  ready_state: number;
  network_state: number;
};

export type FrontendAudioEvents = {
  status_change?: (status: PlaybackStatus) => void;
  ended?: (status: PlaybackStatus) => void;
  error?: (error: FrontendAudioError) => void;
};

export class FrontendAudioPlayInterruptedError extends Error {
  constructor() {
    super("播放请求已被新的播放加载打断");
    this.name = "FrontendAudioPlayInterruptedError";
  }
}

export function is_frontend_audio_play_interrupted(error: unknown) {
  if (error instanceof FrontendAudioPlayInterruptedError) return true;
  if (error instanceof DOMException && error.name === "AbortError") return true;
  const message = (error instanceof Error ? error.message : String(error)).toLowerCase();
  return (
    message.includes("play() request was interrupted") ||
    message.includes("interrupted by a new load request") ||
    message.includes("interrupted by a call to pause")
  );
}

export function is_frontend_audio_error_ignorable(error: FrontendAudioError) {
  return error.code === MediaError.MEDIA_ERR_ABORTED || is_frontend_audio_play_interrupted(error.message);
}

export class FrontendAudioPlayer {
  private path: string | null = null;
  private volume = 1;
  private play_request_id = 0;

  constructor(
    private readonly audio: HTMLAudioElement,
    private readonly events: FrontendAudioEvents = {},
  ) {
    this.audio.preload = "auto";
    this.audio.controls = false;
    this.audio.addEventListener("play", () => this.emit_status());
    this.audio.addEventListener("pause", () => this.emit_status());
    this.audio.addEventListener("timeupdate", () => this.emit_status());
    this.audio.addEventListener("ended", () => {
      this.emit_status();
      this.events.ended?.(this.status());
    });
    this.audio.addEventListener("error", () => {
      const error = this.audio_error();
      if (!is_frontend_audio_error_ignorable(error)) {
        this.events.error?.(error);
      }
      this.emit_status();
    });
  }

  async play(track: Track, seconds = 0) {
    const request_id = ++this.play_request_id;
    this.path = track.path;
    this.audio.src = convertFileSrc(track.path);
    this.audio.volume = this.safe_audio_volume(this.volume);
    if (seconds > 0) {
      await this.wait_for_metadata(request_id);
      this.audio.currentTime = Math.max(seconds, 0);
    }
    await this.play_audio_for_request(request_id);
    this.emit_status();
  }

  pause() {
    this.play_request_id += 1;
    this.audio.pause();
    this.emit_status();
  }

  async resume() {
    if (!this.path) return;
    const request_id = ++this.play_request_id;
    await this.play_audio_for_request(request_id);
    this.emit_status();
  }

  stop() {
    this.play_request_id += 1;
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
      elapsed: Math.max(this.audio.currentTime || 0, 0),
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

  private async play_audio_for_request(request_id: number) {
    if (!this.is_current_play_request(request_id)) {
      throw new FrontendAudioPlayInterruptedError();
    }

    try {
      await this.audio.play();
    } catch (error) {
      if (!this.is_current_play_request(request_id) || is_frontend_audio_play_interrupted(error)) {
        throw new FrontendAudioPlayInterruptedError();
      }
      throw error;
    }

    if (!this.is_current_play_request(request_id)) {
      throw new FrontendAudioPlayInterruptedError();
    }
  }

  private wait_for_metadata(request_id: number) {
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
        if (this.is_current_play_request(request_id)) {
          resolve();
        } else {
          reject(new FrontendAudioPlayInterruptedError());
        }
      };
      const handle_error = () => {
        cleanup();
        if (this.is_current_play_request(request_id)) {
          reject(new Error(this.audio_error_message()));
        } else {
          reject(new FrontendAudioPlayInterruptedError());
        }
      };
      this.audio.addEventListener("loadedmetadata", handle_loaded, { once: true });
      this.audio.addEventListener("error", handle_error, { once: true });
      this.audio.load();
    });
  }

  private is_current_play_request(request_id: number) {
    return request_id === this.play_request_id;
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

  private audio_error(): FrontendAudioError {
    return {
      path: this.path,
      source: this.audio.currentSrc || this.audio.src,
      code: this.audio.error?.code ?? null,
      message: this.audio_error_message(),
      elapsed: Math.max(Math.floor(this.audio.currentTime || 0), 0),
      ready_state: this.audio.readyState,
      network_state: this.audio.networkState,
    };
  }
}
