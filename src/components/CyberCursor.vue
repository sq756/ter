<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { globalState } from '../store';

const mouseX = ref(-100);
const mouseY = ref(-100);
const isHovering = ref(false);
const isPressed = ref(false);

const config = computed(() => globalState.cursorConfig);

const updateMouse = (e: MouseEvent) => {
  mouseX.value = e.clientX;
  mouseY.value = e.clientY;
  
  // Detect if hovering over clickable elements
  const target = e.target as HTMLElement;
  isHovering.value = !!target.closest('button, a, input, select, .tab-item, .node-info, [role="button"]');
};

const updateTouch = (e: TouchEvent) => {
  if (e.touches.length > 0) {
    mouseX.value = e.touches[0].clientX;
    mouseY.value = e.touches[0].clientY;
  }
};

const handleMouseDown = () => isPressed.value = true;
const handleMouseUp = () => isPressed.value = false;

onMounted(() => {
  window.addEventListener('mousemove', updateMouse, { passive: true });
  window.addEventListener('mousedown', handleMouseDown);
  window.addEventListener('mouseup', handleMouseUp);
  window.addEventListener('touchstart', updateTouch, { passive: true });
});

onUnmounted(() => {
  window.removeEventListener('mousemove', updateMouse);
  window.removeEventListener('mousedown', handleMouseDown);
  window.removeEventListener('mouseup', handleMouseUp);
  window.removeEventListener('touchstart', updateTouch);
});

const cursorStyle = computed(() => ({
  transform: `translate(${mouseX.value}px, ${mouseY.value}px) scale(${isPressed.value ? 0.8 : (isHovering.value ? 1.5 : 1)})`,
  width: `${config.value.size}px`,
  height: `${config.value.size}px`,
  marginTop: `-${config.value.size / 2}px`,
  marginLeft: `-${config.value.size / 2}px`,
  backgroundColor: config.value.color,
  boxShadow: `0 0 ${config.value.glow}px ${config.value.color}`,
  opacity: config.value.enabled ? 1 : 0
}));
</script>

<template>
  <div v-if="config.enabled" class="cyber-cursor-wrapper">
    <div class="cyber-cursor" :style="cursorStyle" :class="{ 'breathing': config.breathing }"></div>
  </div>
</template>

<style scoped>
.cyber-cursor-wrapper {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  pointer-events: none;
  z-index: 999999;
}

.cyber-cursor {
  position: absolute;
  top: 0;
  left: 0;
  border-radius: 50%;
  transition: transform 0.08s linear, opacity 0.3s;
  will-change: transform;
}

.breathing {
  animation: cursor-breath 2s infinite ease-in-out;
}

@keyframes cursor-breath {
  0%, 100% { filter: brightness(1) blur(0px); }
  50% { filter: brightness(1.8) blur(2px); transform: translate(var(--tw-translate-x), var(--tw-translate-y)) scale(1.2); }
}
</style>
