<script setup lang="ts">
type ScanDialogStatus = "loading" | "success" | "error";

defineProps<{
  status: ScanDialogStatus;
  title: string;
  message: string;
  detail?: string;
}>();

const emit = defineEmits<{
  confirm: [];
}>();
</script>

<template>
  <div class="scan_overlay">
    <section
      class="scan_dialog"
      role="status"
      aria-live="polite"
      aria-modal="true"
      :aria-label="title"
    >
      <div class="scan_icon" :class="status">
        <span v-if="status === 'loading'" class="scan_spinner" />
        <span v-else-if="status === 'success'">✓</span>
        <span v-else>!</span>
      </div>
      <div class="scan_content">
        <h2>{{ title }}</h2>
        <p>{{ message }}</p>
        <span v-if="detail">{{ detail }}</span>
      </div>
      <div v-if="status !== 'loading'" class="scan_actions">
        <button
          class="scan_confirm"
          :class="{ success: status === 'success' }"
          type="button"
          @click="emit('confirm')"
        >
          确认
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.scan_overlay {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(22, 24, 29, 0.28);
}

.scan_dialog {
  display: grid;
  justify-items: center;
  gap: 18px;
  width: min(440px, 100%);
  border: 1px solid #e7ebf3;
  border-radius: 8px;
  padding: 26px 24px 22px;
  background: #ffffff;
  box-shadow: 0 22px 70px rgba(19, 24, 34, 0.2);
}

.scan_icon {
  display: grid;
  width: 46px;
  height: 46px;
  place-items: center;
  border-radius: 50%;
  font-size: 1.25rem;
  font-weight: 900;
}

.scan_icon.loading {
  color: #426dff;
  background: #eaf0ff;
}

.scan_icon.success {
  color: #1f8f4d;
  background: #e9f8ef;
}

.scan_icon.error {
  color: #c33131;
  background: #fff0f0;
}

.scan_spinner {
  width: 22px;
  height: 22px;
  border: 3px solid rgba(66, 109, 255, 0.2);
  border-top-color: #426dff;
  border-radius: 50%;
  animation: scan_spin 0.75s linear infinite;
}

.scan_content {
  min-width: 0;
  text-align: center;
}

.scan_content h2 {
  margin: 0 0 8px;
  color: #1e2026;
  font-size: 1.08rem;
  font-weight: 900;
}

.scan_content p {
  margin: 0;
  color: #4a4f59;
  font-size: 0.95rem;
  font-weight: 800;
  line-height: 1.6;
}

.scan_content span {
  display: block;
  margin-top: 8px;
  color: #8b919c;
  font-size: 0.86rem;
  font-weight: 700;
}

.scan_actions {
  display: flex;
  justify-content: center;
  width: 100%;
  margin-top: 2px;
}

.scan_confirm {
  min-width: 88px;
  min-height: 36px;
  border: 0;
  border-radius: 8px;
  padding: 0 16px;
  color: #ffffff;
  background: #426dff;
  font-size: 0.92rem;
  font-weight: 900;
  cursor: pointer;
}

.scan_confirm.success {
  background: #22a05a;
}

.scan_confirm.success:hover {
  background: #1d8d4f;
}

@keyframes scan_spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
