import { defineStore } from "pinia";

export type NotificationType = "info" | "success" | "error" | "warning";

export type NotificationItem = {
  id: number;
  type: NotificationType;
  message: string;
};

let next_notification_id = 1;
const notification_timers = new Map<number, number>();

export const use_notification_store = defineStore("notifications", {
  state: () => ({
    items: [] as NotificationItem[],
  }),
  actions: {
    show(message: string, type: NotificationType = "info", duration = 5000) {
      const id = next_notification_id++;
      this.items.push({ id, type, message });

      if (duration > 0) {
        const timer = window.setTimeout(() => {
          this.remove(id);
        }, duration);
        notification_timers.set(id, timer);
      }

      return id;
    },
    info(message: string, duration = 5000) {
      return this.show(message, "info", duration);
    },
    success(message: string, duration = 5000) {
      return this.show(message, "success", duration);
    },
    error(message: string, duration = 5000) {
      return this.show(message, "error", duration);
    },
    warning(message: string, duration = 5000) {
      return this.show(message, "warning", duration);
    },
    remove(id: number) {
      const timer = notification_timers.get(id);
      if (timer) {
        window.clearTimeout(timer);
        notification_timers.delete(id);
      }
      this.items = this.items.filter((item) => item.id !== id);
    },
    clear() {
      for (const timer of notification_timers.values()) {
        window.clearTimeout(timer);
      }
      notification_timers.clear();
      this.items = [];
    },
  },
});
