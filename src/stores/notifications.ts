import { defineStore } from "pinia";
import { ref } from "vue";

export type NotificationType = "info" | "success" | "error" | "warning";

export type NotificationItem = {
  id: number;
  type: NotificationType;
  message: string;
};

let next_notification_id = 1;
const notification_timers = new Map<number, number>();

export const use_notification_store = defineStore("notifications", () => {
  const items = ref<NotificationItem[]>([]);

  function show(message: string, type: NotificationType = "info", duration = 5000) {
    const id = next_notification_id++;
    items.value.push({ id, type, message });

    if (duration > 0) {
      const timer = window.setTimeout(() => {
        remove(id);
      }, duration);
      notification_timers.set(id, timer);
    }

    return id;
  }

  function info(message: string, duration = 5000) {
    return show(message, "info", duration);
  }

  function success(message: string, duration = 5000) {
    return show(message, "success", duration);
  }

  function error(message: string, duration = 5000) {
    return show(message, "error", duration);
  }

  function warning(message: string, duration = 5000) {
    return show(message, "warning", duration);
  }

  function remove(id: number) {
    const timer = notification_timers.get(id);
    if (timer) {
      window.clearTimeout(timer);
      notification_timers.delete(id);
    }
    items.value = items.value.filter((item) => item.id !== id);
  }

  function clear() {
    for (const timer of notification_timers.values()) {
      window.clearTimeout(timer);
    }
    notification_timers.clear();
    items.value = [];
  }

  return {
    items,
    show,
    info,
    success,
    error,
    warning,
    remove,
    clear,
  };
});
