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
  background-color: var(--notification-bg);
  font-size: 0.92rem;
  font-weight: 800;
  text-align: center;
}

.global_notification_info {
  --notification-bg: rgb(64 158 255 / 0.12);
  --notification-border: rgb(64 158 255 / 0.2);
  --notification-text: #409eff;
}

.global_notification_success {
  --notification-bg: rgb(103 194 58 / 0.12);
  --notification-border: rgb(103 194 58 / 0.2);
  --notification-text: #67c23a;
}

.global_notification_error {
  --notification-bg: rgb(245 108 108 / 0.12);
  --notification-border: rgb(245 108 108 / 0.2);
  --notification-text: #f56c6c;
}

.global_notification_warning {
  --notification-bg: rgb(230 162 60 / 0.14);
  --notification-border: rgb(230 162 60 / 0.22);
  --notification-text: #e6a23c;
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
