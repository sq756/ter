<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';

// Basic styles for VueFlow
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';

const props = defineProps<{
  activeId: string | null;
}>();

const emit = defineEmits(['close']);

const { onConnect, addEdges, fitView } = useVueFlow();

const nodes = ref<any[]>([]);
const edges = ref<any[]>([]);

const buildTopology = async () => {
  if (!props.activeId) return;
  
  try {
    const chain = await invoke<any[]>('get_connection_chain', { id: props.activeId });
    
    const newNodes = [];
    const newEdges = [];
    
    // Add Localhost
    newNodes.push({
      id: 'local',
      type: 'input',
      label: 'LOCALHOST',
      position: { x: 50, y: 200 },
      style: { background: '#09090b', color: '#22c55e', border: '1px solid #22c55e', borderRadius: '4px', fontSize: '10px' },
    });

    let prevId = 'local';
    chain.forEach((server, index) => {
      const isTarget = index === chain.length - 1;
      const nodeId = `node-${server.id}`;
      
      newNodes.push({
        id: nodeId,
        type: isTarget ? 'output' : 'default',
        label: (server.label || server.host).toUpperCase(),
        position: { x: 250 + index * 200, y: 200 },
        style: { 
          background: '#09090b', 
          color: isTarget ? '#a855f7' : '#3b82f6', 
          border: `1px solid ${isTarget ? '#a855f7' : '#3b82f6'}`, 
          borderRadius: '4px', 
          fontSize: '10px',
          boxShadow: `0 0 10px ${isTarget ? 'rgba(168, 85, 247, 0.3)' : 'rgba(59, 130, 246, 0.3)'}`
        },
      });

      newEdges.push({
        id: `e-${prevId}-${nodeId}`,
        source: prevId,
        target: nodeId,
        animated: true,
        style: { stroke: '#22c55e' }
      });

      prevId = nodeId;
    });

    nodes.value = newNodes;
    edges.value = newEdges;
    
    setTimeout(() => fitView(), 100);
  } catch (e) {
    console.error("Topology fail", e);
  }
};

onMounted(buildTopology);

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
