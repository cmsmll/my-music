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
    @contextmenu.stop.prevent
  >
    <slot />
  </div>
</template>

<style scoped>
.context_menu {
  position: fixed;
  z-index: 1000;
  display: grid;
  gap: 3px;
  overflow: hidden;
  border: 1px solid #e7eaf1;
  border-radius: 6px;
  padding: 6px;
  color: #20242c;
  background: #ffffff;
  box-shadow: 0 10px 28px rgba(17, 24, 39, 0.14);
}
</style>
