<script setup lang="ts">
defineProps<{
  x: number;
  y: number;
}>();
</script>

<template>
  <div
    class="context_menu"
    :style="{ left: `${x}px`, top: `${y}px` }"
    @click.stop
    @contextmenu.prevent
  >
    <slot />
  </div>
</template>

<style scoped>
.context_menu {
  position: fixed;
  z-index: 1000;
  display: grid;
  gap: 4px;
  border: 1px solid #eef0f4;
  border-radius: 8px;
  padding: 8px;
  background: #ffffff;
  box-shadow: 0 12px 34px rgba(19, 24, 34, 0.16);
}
</style>
