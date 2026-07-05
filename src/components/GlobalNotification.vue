<script setup lang="ts">
import { storeToRefs } from "pinia";
import { use_notification_store } from "../stores/notifications";

const notification_store = use_notification_store();
const { items } = storeToRefs(notification_store);
</script>

<template>
  <TransitionGroup name="global_notification" tag="div" class="global_notification_layer">
    <div
      v-for="item in items"
      :key="item.id"
      class="global_notification"
      :class="`global_notification_${item.type}`"
      role="status"
    >
      {{ item.message }}
    </div>
  </TransitionGroup>
</template>

<style scoped>
.global_notification_layer {
  position: fixed;
  top: 18px;
  left: 50%;
  z-index: 2000;
  display: grid;
  gap: 8px;
  width: min(420px, calc(100vw - 32px));
  pointer-events: none;
  transform: translateX(-50%);
}

.global_notification {
  min-height: 38px;
  border: 1px solid var(--notification-border);
  border-radius: 6px;
  padding: 8px 14px;
  color: var(--notification-text);
  background: var(--notification-bg);
  box-shadow: 0 12px 30px rgba(22, 24, 29, 0.14);
  font-size: 0.92rem;
  font-weight: 800;
  text-align: center;
}

.global_notification_info {
  --notification-bg: #eef5ff;
  --notification-border: #6fa8ff;
  --notification-text: #245ebd;
}

.global_notification_success {
  --notification-bg: #ecf9f1;
  --notification-border: #8bdca8;
  --notification-text: #1f8c4c;
}

.global_notification_error {
  --notification-bg: #fff0f0;
  --notification-border: #ffb6b6;
  --notification-text: #c24747;
}

.global_notification_warning {
  --notification-bg: #fff8dd;
  --notification-border: #f0cf66;
  --notification-text: #9a6b00;
}

.global_notification-enter-active,
.global_notification-leave-active {
  transition:
    opacity 0.18s ease,
    transform 0.18s ease;
}

.global_notification-enter-from,
.global_notification-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
