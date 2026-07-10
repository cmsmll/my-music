import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { ref } from "vue";
import type { AppConfig, AppStateConfig, DecoderConfig, StyleConfig } from "../types/music";

let save_timer: ReturnType<typeof setTimeout> | undefined;
let config_revision = 0;
let save_promise: Promise<void> | null = null;
let save_requested = false;

function clone_config(config: AppConfig): AppConfig {
  return {
    ...config,
    music_directory: [...config.music_directory],
    decoder: {
      ...config.decoder,
      scan_directory: [...config.decoder.scan_directory],
    },
    cache: { ...config.cache },
    style: { ...config.style },
    state: { ...config.state },
  };
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

export const use_app_config_store = defineStore("app_config", () => {
  const config = ref<AppConfig | null>(null);
  const default_config = ref<AppConfig | null>(null);
  const saving = ref(false);
  const save_error = ref("");

  function hydrate_config(next_config: AppConfig, next_default_config?: AppConfig) {
    config.value = clone_config(next_config);
    config_revision += 1;
    if (next_default_config) {
      default_config.value = clone_config(next_default_config);
    }
    save_error.value = "";
  }

  function update_config(patch: Partial<AppConfig> | ((config: AppConfig) => AppConfig)) {
    if (!config.value) return;

    config.value =
      typeof patch === "function"
        ? patch(clone_config(config.value))
        : merge_config(config.value, patch);
    config_revision += 1;
    schedule_config_save();
  }

  function update_decoder(patch: Partial<DecoderConfig>) {
    if (!config.value) return;

    update_config({
      decoder: {
        ...config.value.decoder,
        ...patch,
      },
    });
  }

  function update_style(patch: Partial<StyleConfig>) {
    if (!config.value) return;

    update_config({
      style: {
        ...config.value.style,
        ...patch,
      },
    });
  }

  function update_state(patch: Partial<AppStateConfig>) {
    if (!config.value) return;

    update_config({
      state: {
        ...config.value.state,
        ...patch,
      },
    });
  }

  function schedule_config_save() {
    if (save_timer) {
      window.clearTimeout(save_timer);
    }

    save_timer = window.setTimeout(() => {
      save_timer = undefined;
      void save_config();
    }, 1000);
  }

  async function flush_config_save() {
    if (save_timer) {
      window.clearTimeout(save_timer);
      save_timer = undefined;
    }

    await save_config();
  }

  async function save_config() {
    if (!config.value) return;

    save_requested = true;
    if (!save_promise) {
      save_promise = run_save_loop().finally(() => {
        save_promise = null;
      });
    }
    await save_promise;
  }

  async function run_save_loop() {
    saving.value = true;
    try {
      while (save_requested && config.value) {
        save_requested = false;
        const snapshot = clone_config(config.value);
        const snapshot_revision = config_revision;
        save_error.value = "";

        try {
          const saved_config = await invoke<AppConfig>("update_app_config", {
            config: snapshot,
          });
          if (snapshot_revision === config_revision) {
            config.value = clone_config(saved_config);
          } else {
            save_requested = true;
          }
        } catch (error) {
          save_error.value = String(error);
        }
      }
    } finally {
      saving.value = false;
    }
  }

  return {
    config,
    default_config,
    saving,
    save_error,
    hydrate_config,
    update_config,
    update_decoder,
    update_style,
    update_state,
    schedule_config_save,
    flush_config_save,
    save_config,
  };
});
