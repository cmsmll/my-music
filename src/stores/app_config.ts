import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type { AppConfig, AppStateConfig, DecoderConfig, StyleConfig } from "../types/music";

let save_timer: ReturnType<typeof setTimeout> | undefined;
let config_revision = 0;

function clone_config(config: AppConfig): AppConfig {
  return JSON.parse(JSON.stringify(config)) as AppConfig;
}

function merge_config(config: AppConfig, patch: Partial<AppConfig>): AppConfig {
  return {
    ...config,
    ...patch,
    cache: {
      ...config.cache,
      ...patch.cache,
    },
    decoder: {
      ...config.decoder,
      ...patch.decoder,
    },
    style: {
      ...config.style,
      ...patch.style,
    },
    state: {
      ...config.state,
      ...patch.state,
    },
  };
}

export const use_app_config_store = defineStore("app_config", {
  state: () => ({
    config: null as AppConfig | null,
    default_config: null as AppConfig | null,
    saving: false,
    save_error: "",
  }),
  actions: {
    hydrate_config(config: AppConfig, default_config?: AppConfig) {
      this.config = clone_config(config);
      config_revision += 1;
      if (default_config) {
        this.default_config = clone_config(default_config);
      }
      this.save_error = "";
    },
    update_config(patch: Partial<AppConfig> | ((config: AppConfig) => AppConfig)) {
      if (!this.config) return;

      this.config =
        typeof patch === "function"
          ? patch(clone_config(this.config))
          : merge_config(this.config, patch);
      config_revision += 1;
      this.schedule_config_save();
    },
    update_decoder(patch: Partial<DecoderConfig>) {
      if (!this.config) return;

      this.update_config({
        decoder: {
          ...this.config.decoder,
          ...patch,
        },
      });
    },
    update_style(patch: Partial<StyleConfig>) {
      if (!this.config) return;

      this.update_config({
        style: {
          ...this.config.style,
          ...patch,
        },
      });
    },
    update_state(patch: Partial<AppStateConfig>) {
      if (!this.config) return;

      this.update_config({
        state: {
          ...this.config.state,
          ...patch,
        },
      });
    },
    schedule_config_save() {
      if (save_timer) {
        window.clearTimeout(save_timer);
      }

      save_timer = window.setTimeout(() => {
        save_timer = undefined;
        void this.save_config();
      }, 1000);
    },
    async flush_config_save() {
      if (save_timer) {
        window.clearTimeout(save_timer);
        save_timer = undefined;
      }

      await this.save_config();
    },
    async save_config() {
      if (!this.config) return;

      const snapshot = clone_config(this.config);
      const snapshot_revision = config_revision;
      this.saving = true;
      this.save_error = "";

      try {
        const saved_config = await invoke<AppConfig>("update_app_config", {
          config: snapshot,
        });
        if (snapshot_revision === config_revision) {
          this.config = saved_config;
        }
      } catch (error) {
        this.save_error = String(error);
      } finally {
        this.saving = false;
      }
    },
  },
});
