<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";

defineProps<{
  title: string;
  message: string;
  detail?: string;
  confirm_label?: string;
  cancel_label?: string;
}>();

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const dialog = ref<HTMLElement | null>(null);

onMounted(async () => {
  await nextTick();
  dialog.value?.focus();
});
</script>

<template>
  <div class="confirm_overlay" @click.self="emit('cancel')">
    <section
      ref="dialog"
      class="confirm_dialog"
      role="alertdialog"
      aria-modal="true"
      :aria-label="title"
      tabindex="-1"
      @keydown.esc.prevent="emit('cancel')"
    >
      <div class="confirm_icon">!</div>
      <div class="confirm_content">
        <h2>{{ title }}</h2>
        <p>{{ message }}</p>
        <span v-if="detail">{{ detail }}</span>
      </div>
      <div class="confirm_actions">
        <button class="confirm_cancel" type="button" @click="emit('cancel')">
          {{ cancel_label ?? "取消" }}
        </button>
        <button class="confirm_danger" type="button" @click="emit('confirm')">
          {{ confirm_label ?? "确认" }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.confirm_overlay {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(22, 24, 29, 0.32);
}

.confirm_dialog {
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr);
  gap: 16px;
  width: min(420px, 100%);
  border: 1px solid #f0d3d3;
  border-radius: 8px;
  padding: 20px;
  background: #ffffff;
  box-shadow: 0 22px 70px rgba(19, 24, 34, 0.24);
  outline: 0;
}

.confirm_icon {
  display: grid;
  width: 44px;
  height: 44px;
  place-items: center;
  border-radius: 50%;
  color: #c33131;
  background: #fff0f0;
  font-size: 1.35rem;
  font-weight: 900;
}

.confirm_content {
  min-width: 0;
}

.confirm_content h2 {
  margin: 0 0 8px;
  color: var(--theme-title-color, #1e2026);
  font-size: 1.05rem;
  font-weight: 900;
}

.confirm_content p {
  margin: 0;
  color: #4a4f59;
  font-size: 0.95rem;
  font-weight: 700;
  line-height: 1.6;
}

.confirm_content span {
  display: block;
  margin-top: 8px;
  color: #9a4b4b;
  font-size: 0.86rem;
  font-weight: 700;
}

.confirm_actions {
  grid-column: 1 / -1;
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 8px;
}

.confirm_actions button {
  min-width: 82px;
  min-height: 36px;
  border-radius: 8px;
  padding: 0 14px;
  font-size: 0.92rem;
  font-weight: 900;
}

.confirm_cancel {
  color: #3c414b;
  background: #f0f2f6;
}

.confirm_cancel:hover {
  background: #e6e9ef;
}

.confirm_danger {
  color: #ffffff;
  background: #c33131;
}

.confirm_danger:hover {
  background: #ae2626;
}
</style>
