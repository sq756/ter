<script setup lang="ts">
import { ref } from 'vue';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';

// Basic styles for VueFlow
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';

const emit = defineEmits(['close']);

const { onConnect, addEdges } = useVueFlow();

const nodes = ref([
  {
    id: '1',
    type: 'input',
    label: 'Localhost',
    position: { x: 100, y: 200 },
    style: { background: '#18181b', color: '#22c55e', border: '1px solid #22c55e', borderRadius: '4px', fontSize: '12px' },
  },
  {
    id: '2',
    label: 'Jump Proxy / VPN',
    position: { x: 400, y: 200 },
    style: { background: '#18181b', color: '#3b82f6', border: '1px solid #3b82f6', borderRadius: '4px', fontSize: '12px' },
  },
  {
    id: '3',
    type: 'output',
    label: 'Target Server',
    position: { x: 700, y: 200 },
    style: { background: '#18181b', color: '#a855f7', border: '1px solid #a855f7', borderRadius: '4px', fontSize: '12px' },
  },
]);

const edges = ref([]);

onConnect((params) => {
  addEdges([params]);
});
</script>

<template>
  <div class="network-matrix-overlay">
    <div class="matrix-header">
      <div class="matrix-title">NETWORK // TOPOLOGY_MATRIX</div>
      <button class="close-btn" @click="$emit('close')">×</button>
    </div>

    <div class="flow-container">
      <VueFlow
        v-model:nodes="nodes"
        v-model:edges="edges"
        :fit-view-on-init="true"
        class="cyber-flow"
      >
        <Background pattern-color="#22c55e" :gap="20" :size="1" />
      </VueFlow>
    </div>

    <div class="matrix-footer">
      /// Drag nodes to configure proxy chains. Connect dots for reverse tunneling.
    </div>
  </div>
</template>

<style scoped>
.network-matrix-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.85);
  backdrop-filter: blur(10px);
  z-index: 100000;
  display: flex;
  flex-direction: column;
  color: #d4d4d8;
  font-family: 'JetBrains Mono', monospace;
}

.matrix-header {
  height: 50px;
  padding: 0 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid rgba(34, 197, 94, 0.2);
}

.matrix-title {
  font-size: 14px;
  letter-spacing: 2px;
  color: #22c55e;
  font-weight: bold;
}

.close-btn {
  background: transparent;
  border: none;
  color: #52525b;
  font-size: 24px;
  cursor: pointer;
  transition: color 0.2s;
}

.close-btn:hover {
  color: #ef4444;
}

.flow-container {
  flex: 1;
  width: 100%;
  position: relative;
}

.cyber-flow {
  background: transparent;
}

/* Customize Vue Flow internal styles */
:deep(.vue-flow__node) {
  padding: 10px;
  min-width: 150px;
  text-align: center;
  box-shadow: 0 0 15px rgba(0, 0, 0, 0.5);
}

:deep(.vue-flow__edge-path) {
  stroke: #22c55e;
  stroke-width: 2;
}

:deep(.vue-flow__handle) {
  background: #22c55e;
  width: 8px;
  height: 8px;
  border: 1px solid #000;
}

.matrix-footer {
  height: 30px;
  padding: 0 20px;
  display: flex;
  align-items: center;
  font-size: 10px;
  color: #166534;
  border-top: 1px solid rgba(34, 197, 94, 0.1);
}
</style>
