<script setup lang="ts">
import TerminalView from './TerminalView.vue';
import { terminalManager } from '../TerminalManager';

import { getCurrentWindow } from '@tauri-apps/api/window';
const appWindow = getCurrentWindow();

const props = defineProps<{
  tabs: any[];
  activeTabId: string | null;
  connectionStatus: 'connected' | 'busy' | 'disconnected';
}>();

defineEmits(['switch-tab', 'close-tab', 'new-tab', 'terminal-context', 'rename-tab', 'pin-tab', 'copy-tab-id']);

const getVisibleTabs = () => props.tabs.filter(t => !t.isBackground);

const minimize = () => appWindow.minimize();
const toggleMaximize = () => appWindow.toggleMaximize();
const closeApp = () => appWindow.close();
</script>

<template>
  <div class="terminal-workspace">
    <!-- Multi-Terminal Tab Bar -->
    <nav class="tab-bar" data-tauri-drag-region>
      <!-- Status Indicator & Quick Switcher -->
      <div class="status-indicator-zone" @click="$emit('new-tab')" title="Quick Command (Ctrl+T)">
        <div class="status-dot" :class="connectionStatus"></div>
        <div class="quick-switcher-icon">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
        </div>
      </div>

      <div v-for="t in getVisibleTabs()" 
           :key="t.id" 
           class="tab-item" 
           :class="{ 'active': t.id === activeTabId }" 
           @click="$emit('switch-tab', t.id)"
           @contextmenu.prevent="$emit('terminal-context', { e: $event, id: t.id })">
        <span class="tab-icon">🐚</span>
        <span class="title">{{ t.title }}</span>
        <button class="btn-close" @click.stop="$emit('close-tab', t.id)">×</button>
        <div class="active-bar" v-if="t.id === activeTabId"></div>
      </div>
      <button class="btn-new-tab" @click="$emit('new-tab')" title="New Terminal (Ctrl+T)">+</button>

      <!-- v2.11.26: Stealth Window Controls -->
      <div class="window-controls">
        <button class="win-btn" @click="minimize" title="Minimize">—</button>
        <button class="win-btn" @click="toggleMaximize" title="Maximize">⬜</button>
        <button class="win-btn close" @click="closeApp" title="Close">✕</button>
      </div>
    </nav>

    <div class="workspace-body">
      <section class="terminal-pane">
        <!-- Persistent Terminal Views: Preserve physical instance with v-show -->
        <div v-for="t in tabs" :key="t.id" 
             class="terminal-wrapper"
             v-show="t.id === activeTabId"
             @click="terminalManager.focus(t.id)"
             @contextmenu.prevent="$emit('terminal-context', { e: $event, id: t.id })">
          <TerminalView :id="t.id" :active="t.id === activeTabId" />
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.terminal-workspace { 
  flex: 1; 
  display: flex; 
  flex-direction: column; 
  height: 100%; 
  overflow: hidden; 
  background: #09090b; 
  position: relative; 
}

.tab-bar { 
  background: #09090b; 
  border-bottom: 1px solid #27272a; 
  display: flex; 
  align-items: center; 
  padding: 0; 
  height: 36px; 
  flex-shrink: 0; 
  z-index: 10; 
  overflow-x: auto !important;
  overflow-y: hidden;
  white-space: nowrap;
}

.tab-bar::-webkit-scrollbar { height: 4px; }
.tab-bar::-webkit-scrollbar-thumb { background: #333; border-radius: 2px; }

.status-indicator-zone {
  padding: 0 12px;
  display: flex;
  align-items: center;
  gap: 12px;
  border-right: 1px solid #27272a;
  height: 100%;
  cursor: pointer;
  transition: background 0.2s;
}

.status-indicator-zone:hover {
  background: rgba(34, 197, 94, 0.05);
}

.status-indicator-zone:hover .quick-switcher-icon {
  color: #22c55e;
}

.quick-switcher-icon {
  color: #52525b;
  display: flex;
  align-items: center;
  transition: color 0.2s;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #52525b;
  transition: all 0.3s;
}

.status-dot.connected {
  background: #3b82f6;
  box-shadow: 0 0 10px #3b82f6;
  animation: pulse-blue 2s infinite;
}

.status-dot.busy {
  background: #a855f7;
  box-shadow: 0 0 10px #a855f7;
  animation: pulse-purple 0.5s infinite;
}

@keyframes pulse-blue {
  0% { opacity: 0.6; transform: scale(0.9); }
  50% { opacity: 1; transform: scale(1.1); }
  100% { opacity: 0.6; transform: scale(0.9); }
}

@keyframes pulse-purple {
  0% { opacity: 0.8; transform: scale(1); }
  50% { opacity: 1; transform: scale(1.2); }
  100% { opacity: 0.8; transform: scale(1); }
}

.tab-item { 
  padding: 0 16px; 
  height: 100%; 
  display: flex; 
  align-items: center; 
  font-size: 12px; 
  color: #52525b; 
  cursor: pointer; 
  position: relative; 
  min-width: 120px; 
  max-width: 200px;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1); 
  border-right: 1px solid #18181b;
  flex-shrink: 0 !important;
}

.tab-icon {
  margin-right: 8px;
  font-size: 12px;
  opacity: 0.5;
}

.tab-item.active { 
  color: #fafafa; 
  background: rgba(255, 255, 255, 0.02);
}

.tab-item:hover:not(.active) { 
  background: rgba(255, 255, 255, 0.04); 
  color: #a1a1aa; 
}

.active-bar {
  position: absolute;
  bottom: 0;
  left: 0;
  width: 100%;
  height: 2px;
  background: #3b82f6;
  box-shadow: 0 0 10px #3b82f6;
}

.tab-item .btn-close { 
  position: absolute;
  right: 8px;
  background: transparent; 
  border: none; 
  color: #52525b; 
  cursor: pointer; 
  opacity: 0; 
  font-size: 14px; 
  transition: opacity 0.2s;
}

.tab-item:hover .btn-close { 
  opacity: 1; 
}

.tab-item .btn-close:hover {
  color: #ef4444;
}

.btn-new-tab { 
  background: transparent; 
  border: none; 
  color: #52525b; 
  padding: 0 12px; 
  cursor: pointer; 
  font-size: 18px; 
  height: 100%;
  transition: all 0.2s;
}

.btn-new-tab:hover {
  color: #fafafa;
  background: rgba(255, 255, 255, 0.05);
}

.window-controls {
  margin-left: auto;
  display: flex;
  height: 100%;
}

.win-btn {
  width: 44px;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: #52525b;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.2s;
  -webkit-app-region: no-drag;
}

.win-btn:hover {
  background: rgba(255, 255, 255, 0.05);
  color: #22c55e;
  text-shadow: 0 0 8px #22c55e;
}

.win-btn.close:hover {
  background: #ef4444;
  color: #fff;
  text-shadow: none;
}

.workspace-body { flex: 1; position: relative; overflow: hidden; display: flex; }
.terminal-pane { 
  height: 100%; 
  flex: 1;
  position: relative; 
  background: #09090b; 
}

.terminal-wrapper {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}
</style>
