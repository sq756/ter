<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { terminalManager } from '../TerminalManager';

const props = defineProps<{
  id: string;
  active: boolean;
}>();

const terminalRef = ref<HTMLElement | null>(null);
let resizeObserver: ResizeObserver | null = null;

const initTerminal = async () => {
  if (!terminalRef.value) return;
  
  const instance = terminalManager.getOrCreate(props.id);
  const { term, fit } = instance;

  // Ensure DOM is ready and element is visible
  await nextTick();

  // If already opened elsewhere, xterm handles it, but we should clear local DOM
  if (term.element && term.element !== terminalRef.value) {
    console.log(`[TerminalView] Terminal ${props.id} moving to new element`);
    if (term.element.parentElement) {
      term.element.parentElement.innerHTML = '';
    }
  }
  
  terminalRef.value.innerHTML = '';
  term.open(terminalRef.value);

  // Resize Handling
  const performFit = () => {
    if (props.active && terminalRef.value && terminalRef.value.offsetWidth > 0) {
      console.log(`[TerminalView] Fitting terminal ${props.id}`);
      fit.fit();
    }
  };

  resizeObserver = new ResizeObserver(() => {
    performFit();
  });
  resizeObserver.observe(terminalRef.value);

  // Immediate Fit
  performFit();
  if (props.active) {
    term.focus();
  }

  // Layout stabilization retries
  setTimeout(performFit, 100);
  setTimeout(performFit, 500);
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
    console.log(`[TerminalView] Terminal ${props.id} became active`);
    const { term, fit } = terminalManager.getOrCreate(props.id);
    nextTick(() => {
      if (terminalRef.value && terminalRef.value.offsetWidth > 0) {
        fit.fit();
        term.focus();
      }
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
  min-height: 100px;
  min-width: 100px;
  background: #000;
  overflow: hidden;
  position: relative;
}

/* 
 * CRITICAL: Hide the xterm.js focus capture box. 
 * This prevents the 'left-corner white box' from showing up.
 */
.xterm-helper-textarea {
  position: absolute !important;
  opacity: 0 !important;
  left: -9999px !important;
  pointer-events: none !important;
}

.terminal-view-container .xterm {
  padding: 10px;
  height: 100%;
}

.terminal-view-container .xterm-viewport {
  background-color: #000 !important;
}
</style>
