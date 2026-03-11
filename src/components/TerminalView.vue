<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { terminalManager, WebglAddon } from '../TerminalManager';

const props = defineProps<{
  id: string;
  active: boolean;
}>();

const terminalRef = ref<HTMLElement | null>(null);
let resizeObserver: ResizeObserver | null = null;

const initTerminal = () => {
  if (!terminalRef.value) return;
  
  const instance = terminalManager.getOrCreate(props.id);
  const { term, fit } = instance;

  // Clear container to prevent duplicate canvas
  terminalRef.value.innerHTML = '';
  
  // Re-open/mount to current element
  term.open(terminalRef.value);

  // Load WebGL only if needed and term is mounted
  if (!(instance as any).webgl) {
    try {
      const webgl = new WebglAddon();
      term.loadAddon(webgl);
      (instance as any).webgl = webgl;
    } catch (e) {
      console.warn("WebGL failed for terminal", props.id, e);
    }
  }

  // Setup Observer
  resizeObserver = new ResizeObserver(() => {
    if (props.active) {
      fit.fit();
    }
  });
  resizeObserver.observe(terminalRef.value);

  // Initial fit
  requestAnimationFrame(() => {
    fit.fit();
    if (props.active) term.focus();
  });
};

onMounted(() => {
  initTerminal();
});

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect();
  }
});

watch(() => props.active, (isActive) => {
  if (isActive) {
    const { term, fit } = terminalManager.getOrCreate(props.id);
    requestAnimationFrame(() => {
      fit.fit();
      term.focus();
    });
  }
});
</script>

<template>
  <div ref="terminalRef" class="terminal-view-container"></div>
</template>

<style>
.terminal-view-container {
  width: 100%;
  height: 100%;
  position: relative;
  background: #000;
  overflow: hidden;
}

/* 
 * CRITICAL: Hide the xterm.js focus capture box. 
 * This prevents the 'left-corner white box' from showing up.
 */
.xterm-helper-textarea {
  position: fixed !important;
  left: -9999px !important;
  top: -9999px !important;
  opacity: 0 !important;
}

/* Force xterm internal elements to fill container */
.terminal-view-container .xterm, 
.terminal-view-container .xterm-viewport, 
.terminal-view-container .xterm-screen,
.terminal-view-container canvas {
  display: block !important;
  width: 100% !important;
  height: 100% !important;
}
</style>
