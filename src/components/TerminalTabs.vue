<script setup lang="ts">
import { nextTick } from 'vue';
import TerminalView from './TerminalView.vue';
import TerEditor from './TerEditor.vue';
import CyberPdfViewer from './CyberPdfViewer.vue';
import { terminalManager } from '../TerminalManager';

import { getCurrentWindow } from '@tauri-apps/api/window';
const appWindow = getCurrentWindow();

const props = defineProps<{
  tabs: any[];
  activeTabId: string | null;
  activeTabIdSecondary: string | null;
  splitMode: boolean;
  connectionStatus: 'connected' | 'busy' | 'disconnected';
  uiScale: number;
}>();

const emit = defineEmits(['switch-tab', 'switch-tab-secondary', 'close-tab', 'new-tab', 'terminal-context', 'rename-tab', 'pin-tab', 'copy-tab-id', 'toggle-split', 'save-complete']);

const getVisibleTabs = () => props.tabs.filter(t => !t.isBackground);

const minimize = () => appWindow.minimize();
const toggleMaximize = () => appWindow.toggleMaximize();
const closeApp = () => appWindow.close();

const switchTab = (id: string) => {
  emit('switch-tab', id);
  nextTick(() => {
    terminalManager.focus(id);
  });
};

const switchTabSecondary = (id: string) => {
  emit('switch-tab-secondary', id);
  nextTick(() => {
    terminalManager.focus(id);
  });
};
</script>

<template>
  <div class="terminal-workspace">
    <!-- Multi-Terminal Tab Bar -->
    <nav class="tab-bar">
      <div class="tab-bar-content">
        <!-- Status Indicator & Quick Switcher -->
        <div class="status-indicator-zone" @click="$emit('new-tab')">
          <div class="status-dot" :class="connectionStatus"></div>
          <div class="quick-switcher-icon">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
          </div>
        </div>

        <div class="tabs-scroll-area">
          <div v-for="t in getVisibleTabs()" 
               :key="t.id" 
               class="tab-item" 
               :class="{ 'active': t.id === activeTabId || (splitMode && t.id === activeTabIdSecondary) }" 
               @click.stop="splitMode && activeTabId ? switchTabSecondary(t.id) : switchTab(t.id)"
               @contextmenu.prevent.stop="$emit('terminal-context', { e: $event, id: t.id })">
            <span class="tab-icon">
              <svg v-if="t.viewType === 'terminal'" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line></svg>
              <svg v-else-if="t.viewType === 'webview'" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10z"></path></svg>
              <svg v-else viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg>
            </span>
            <span class="title">{{ t.title }}</span>
            <button class="btn-close" @click.stop="$emit('close-tab', t.id)">
              <svg viewBox="0 0 24 24" width="10" height="10" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
            </button>
            <div class="active-bar" v-if="t.id === activeTabId"></div>
            <div class="active-bar-secondary" v-if="splitMode && t.id === activeTabIdSecondary"></div>
          </div>
        </div>

        <button class="btn-new-tab" @click.stop="$emit('new-tab')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
        </button>
        <button class="btn-split" :class="{ 'active': splitMode }" @click.stop="$emit('toggle-split')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><line x1="12" y1="3" x2="12" y2="21"></line></svg>
        </button>

        <!-- v2.14.22: Re-enabling pure drag spacer to fix interaction dead zones -->
        <div class="drag-spacer" data-tauri-drag-region></div>

        <!-- v2.11.29: Stealth Window Controls (Isolated) -->
        <div class="window-controls">
          <button class="win-btn" @click.stop="minimize">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          </button>
          <button class="win-btn" @click.stop="toggleMaximize">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect></svg>
          </button>
          <button class="win-btn close" @click.stop="closeApp">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
          </button>
        </div>
      </div>
    </nav>

    <div class="workspace-body" :class="{ 'split-mode': splitMode }">
      <!-- Primary Pane -->
      <section class="pane primary-pane">
        <div v-for="t in tabs" :key="t.id" 
             class="tab-view-wrapper"
             v-show="t.id === activeTabId"
             @click="t.viewType === 'terminal' && terminalManager.focus(t.id)">
          <TerminalView v-if="t.viewType === 'terminal'" 
                        :id="t.id" 
                        :active="t.id === activeTabId" 
                        :uiScale="uiScale"
                        @terminal-context="$emit('terminal-context', $event)" />
          <TerEditor v-else-if="t.viewType === 'editor'" 
                     :id="t.id" 
                     :path="t.data?.path" 
                     :initialContent="t.data?.content"
                     @save-complete="$emit('save-complete')" />
          <CyberPdfViewer v-else-if="t.viewType === 'webview'" 
                          :url="t.data?.url" 
                          :title="t.title" />
        </div>
      </section>

      <!-- Secondary Pane (Visible only in Split Mode) -->
      <section v-if="splitMode" class="pane secondary-pane">
        <div v-for="t in tabs" :key="'sec-' + t.id" 
             class="tab-view-wrapper"
             v-show="t.id === activeTabIdSecondary"
             @click="t.viewType === 'terminal' && terminalManager.focus(t.id)">
          <TerminalView v-if="t.viewType === 'terminal'" 
                        :id="t.id" 
                        :active="t.id === activeTabIdSecondary" 
                        :uiScale="uiScale"
                        @terminal-context="$emit('terminal-context', $event)" />
          <TerEditor v-else-if="t.viewType === 'editor'" 
                     :id="t.id" 
                     :path="t.data?.path" 
                     :initialContent="t.data?.content"
                     @save-complete="$emit('save-complete')" />
          <CyberPdfViewer v-else-if="t.viewType === 'webview'" 
                          :url="t.data?.url" 
                          :title="t.title" />
        </div>
        <div v-if="!activeTabIdSecondary" class="empty-pane-msg">SELECT_TAB_FOR_DECK_2</div>
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
  height: calc(var(--ter-tab-bar-height) * var(--ter-ui-scale)); 
  flex-shrink: 0; 
  z-index: 10; 
  overflow: hidden;
}

.tab-bar-content {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  width: 100%;
  height: 100%;
}


.status-indicator-zone {
  padding: 0 calc(12px * var(--ter-ui-scale));
  display: flex;
  align-items: center;
  gap: calc(12px * var(--ter-ui-scale));
  border-right: 1px solid #18181b;
  height: 100%;
  cursor: pointer;
  flex-shrink: 0;
}

.tabs-scroll-area {
  display: flex;
  height: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  flex: 0 1 auto;
  scrollbar-width: none;
}
.tabs-scroll-area::-webkit-scrollbar { display: none; }

.tab-item { 
  padding: 0 calc(16px * var(--ter-ui-scale)); 
  height: 100%; 
  display: flex; 
  align-items: center; 
  font-size: calc(12px * var(--ter-ui-scale)); 
  color: #52525b; 
  cursor: pointer; 
  position: relative; 
  min-width: calc(100px * var(--ter-ui-scale)); 
  max-width: calc(240px * var(--ter-ui-scale));
  transition: all 0.1s; 
  border-right: 1px solid #18181b;
  flex-shrink: 0 !important;
  white-space: nowrap;
}

.drag-spacer {
  flex: 1;
  height: 100%;
}

.window-controls {
  display: flex;
  height: 100%;
  position: relative;
  z-index: 10;
  flex-shrink: 0;
}

.win-btn {
  width: calc(44px * var(--ter-ui-scale));
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: #00ff9d;
  cursor: pointer;
  font-size: calc(12px * var(--ter-ui-scale));
  transition: all 0.1s;
  text-shadow: 0 0 calc(4px * var(--ter-ui-scale)) rgba(0, 255, 157, 0.4);
}

.win-btn:hover {
  background: rgba(0, 255, 157, 0.1);
  color: #fff;
}

.win-btn.close:hover {
  background: #ef4444;
}

.btn-new-tab { 
  background: transparent; 
  border: none; 
  color: #52525b; 
  padding: 0 12px; 
  cursor: pointer; 
  font-size: 18px; 
  height: 100%;
}

.btn-new-tab:hover { color: #fff; }

.btn-split { background: transparent; border: none; color: #52525b; cursor: pointer; padding: 0 12px; height: 100%; transition: all 0.1s; border-left: 1px solid #18181b; }
.btn-split:hover { color: #fff; }
.btn-split.active { color: #22c55e; background: rgba(34, 197, 94, 0.05); }

.workspace-body { flex: 1; position: relative; overflow: hidden; display: flex; width: 100%; height: 100%; }
.workspace-body.split-mode { flex-direction: row; }

.pane { flex: 1; height: 100%; position: relative; overflow: hidden; min-width: 0; }
.primary-pane { background: #000; }
.secondary-pane { border-left: 1px solid #27272a; background: #000; }

.tab-view-wrapper { position: absolute; inset: 0; width: 100%; height: 100%; }

.empty-pane-msg { height: 100%; display: flex; align-items: center; justify-content: center; color: #3f3f46; font-size: 10px; font-family: 'JetBrains Mono', monospace; letter-spacing: 2px; }

.status-dot { 
  width: 8px; 
  height: 8px; 
  border-radius: 50%; 
  background: #52525b; 
  position: relative;
  transition: all 0.1s;
}
.status-dot.connected { background: #3b82f6; box-shadow: 0 0 10px #3b82f6; }
.status-dot.busy { background: #a855f7; box-shadow: 0 0 10px #a855f7; }
.quick-switcher-icon { color: #52525b; display: flex; align-items: center; }
.tab-icon { margin-right: calc(8px * var(--ter-ui-scale)); font-size: calc(12px * var(--ter-ui-scale)); opacity: 0.5; }
.tab-item.active { color: #fafafa; background: rgba(255, 255, 255, 0.02); }
.active-bar { position: absolute; bottom: 0; left: 0; width: 100%; height: 2px; background: #3b82f6; }
.active-bar-secondary { position: absolute; bottom: 0; left: 0; width: 100%; height: 2px; background: #a855f7; }
.tab-item .btn-close { position: absolute; right: calc(8px * var(--ter-ui-scale)); background: transparent; border: none; color: #52525b; cursor: pointer; opacity: 0; }
.tab-item:hover .btn-close { opacity: 1; }
</style>
