<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { terminalManager } from '../TerminalManager';

// Basic styles for VueFlow
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';

const props = defineProps<{
  activeId: string | null;
  activeTabId: string | null;
}>();

const emit = defineEmits(['close']);

const { onConnect, addEdges, fitView } = useVueFlow();

const nodes = ref<any[]>([]);
const edges = ref<any[]>([]);
const authUrl = ref('https://vpn.pku.edu.cn');
const terminalRef = ref<HTMLElement | null>(null);

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
      position: { x: 50, y: 150 },
      style: { background: '#09090b', color: '#22c55e', border: '1px solid #22c55e', borderRadius: '4px', fontSize: '9px' },
    });

    let prevId = 'local';
    chain.forEach((server, index) => {
      const isTarget = index === chain.length - 1;
      const nodeId = `node-${server.id}`;
      
      newNodes.push({
        id: nodeId,
        type: isTarget ? 'output' : 'default',
        label: (server.label || server.host).toUpperCase(),
        position: { x: 50, y: 250 + index * 100 },
        style: { 
          background: '#09090b', 
          color: isTarget ? '#a855f7' : '#3b82f6', 
          border: `1px solid ${isTarget ? '#a855f7' : '#3b82f6'}`, 
          borderRadius: '4px', 
          fontSize: '9px',
          boxShadow: `0 0 10px ${isTarget ? 'rgba(168, 85, 247, 0.2)' : 'rgba(59, 130, 246, 0.2)'}`
        },
      });

      newEdges.push({
        id: `e-${prevId}-${nodeId}`,
        source: prevId,
        target: nodeId,
        animated: true,
        style: { stroke: '#22c55e', strokeWidth: 2 }
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

onMounted(async () => {
  buildTopology();
  
  // Relocate Terminal to Matrix Pane
  if (props.activeTabId && terminalRef.value) {
    await nextTick();
    terminalManager.mount(props.activeTabId, terminalRef.value);
    setTimeout(() => terminalManager.fitAll(), 500);
  }
});

onUnmounted(() => {
  // Terminal will be re-mounted by TerminalView.vue when it becomes visible again
});

onConnect((params) => {
  addEdges([params]);
});
</script>

<template>
  <div class="network-matrix-overlay" @contextmenu.prevent.stop>
    <div class="matrix-header">
      <div class="matrix-title">NETWORK_COMMAND_CENTER // ORCHESTRATION_v2</div>
      <div class="header-actions">
        <button class="action-btn" @click="buildTopology">REFRESH_PATH</button>
        <button class="close-btn" @click="$emit('close')">✕</button>
      </div>
    </div>

    <div class="dashboard-body">
      <!-- Left: Terminal Pane -->
      <section class="pane terminal-pane">
        <header class="pane-header">ACTIVE_PTY_STREAM</header>
        <div ref="terminalRef" class="terminal-container">
          <div v-if="!activeTabId" class="no-pty">NO_ACTIVE_PTY_SESSION</div>
        </div>
      </section>

      <!-- Center: Topology Pane -->
      <section class="pane topology-pane">
        <header class="pane-header">CONNECTION_PATH_VISUALIZER</header>
        <div class="flow-container">
          <VueFlow v-model:nodes="nodes" v-model:edges="edges" :fit-view-on-init="true" class="cyber-flow">
            <Background pattern-color="#22c55e" :gap="20" :size="0.5" />
          </VueFlow>
        </div>
        <div class="topology-hud">
          <div class="hud-item">ENCRYPTION: AES-256-GCM</div>
          <div class="hud-item">TUNNEL: ACTIVE</div>
        </div>
      </section>

      <!-- Right: Web Auth Pane -->
      <section class="pane auth-pane">
        <header class="pane-header">AUTHENTICATION_GATEWAY</header>
        <div class="auth-controls">
          <input v-model="authUrl" class="auth-input" @keyup.enter="authUrl = $event.target.value" />
          <div class="quick-links">
            <button @click="authUrl = 'https://vpn.pku.edu.cn'">PKU_VPN</button>
            <button @click="authUrl = 'https://vpn.pkusz.edu.cn'">SZ_VPN</button>
          </div>
        </div>
        <div class="webview-container">
          <iframe :src="authUrl" class="auth-iframe"></iframe>
        </div>
      </section>
    </div>

    <div class="matrix-footer">
      /// SYSTEM_READY // ALL_TUNNELS_NOMINAL // SECURE_HANDSHAKE_ESTABLISHED
    </div>
  </div>
</template>

<style scoped>
.network-matrix-overlay {
  position: fixed;
  inset: 0;
  background: #000;
  z-index: 100000;
  display: flex;
  flex-direction: column;
  color: #d4d4d8;
  font-family: 'JetBrains Mono', monospace;
}

.matrix-header {
  height: 45px;
  padding: 0 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid #18181b;
  background: #050505;
}

.matrix-title {
  font-size: 12px;
  letter-spacing: 2px;
  color: #22c55e;
  font-weight: bold;
}

.header-actions { display: flex; gap: 15px; align-items: center; }
.action-btn { background: transparent; border: 1px solid #27272a; color: #71717a; font-size: 10px; padding: 4px 10px; cursor: pointer; }
.action-btn:hover { border-color: #22c55e; color: #22c55e; }

.close-btn { background: transparent; border: none; color: #52525b; font-size: 20px; cursor: pointer; }
.close-btn:hover { color: #ef4444; }

.dashboard-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  padding: 10px;
  gap: 10px;
}

.pane {
  display: flex;
  flex-direction: column;
  background: #09090b;
  border: 1px solid #18181b;
  overflow: hidden;
}

.pane-header {
  background: #111111;
  padding: 6px 12px;
  font-size: 10px;
  color: #52525b;
  border-bottom: 1px solid #18181b;
  letter-spacing: 1px;
}

.terminal-pane { flex: 3; }
.topology-pane { flex: 2; position: relative; }
.auth-pane { flex: 3; }

.terminal-container { flex: 1; background: #000; overflow: hidden; }
.no-pty { height: 100%; display: flex; align-items: center; justify-content: center; color: #3f3f46; font-size: 12px; }

.flow-container { flex: 1; position: relative; }
.cyber-flow { background: transparent; }

.topology-hud {
  padding: 10px;
  background: #050505;
  border-top: 1px solid #18181b;
  display: flex;
  justify-content: space-between;
  font-size: 9px;
  color: #166534;
}

.auth-controls { padding: 10px; border-bottom: 1px solid #18181b; background: #050505; }
.auth-input { width: 100%; background: #000; border: 1px solid #27272a; color: #3b82f6; padding: 6px 10px; font-family: inherit; font-size: 11px; outline: none; margin-bottom: 8px; }
.quick-links { display: flex; gap: 10px; }
.quick-links button { background: transparent; border: 1px solid #18181b; color: #52525b; font-size: 9px; padding: 2px 8px; cursor: pointer; }
.quick-links button:hover { border-color: #3b82f6; color: #3b82f6; }

.webview-container { flex: 1; background: #fff; }
.auth-iframe { width: 100%; height: 100%; border: none; }

.matrix-footer {
  height: 30px;
  padding: 0 20px;
  display: flex;
  align-items: center;
  font-size: 9px;
  color: #166534;
  border-top: 1px solid #18181b;
  background: #050505;
}

:deep(.vue-flow__node) { padding: 8px; min-width: 120px; text-align: center; }
:deep(.vue-flow__edge-path) { stroke: #22c55e; stroke-width: 2; }
:deep(.vue-flow__handle) { background: #22c55e; width: 6px; height: 6px; }
</style>
