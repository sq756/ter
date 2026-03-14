<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const vaultEntries = ref<any[]>([]);
const isLoading = ref(false);

const loadVault = async () => {
  isLoading.value = true;
  try {
    vaultEntries.value = await invoke('list_vault');
  } catch (e) {
    console.error("Failed to load vault", e);
  } finally {
    isLoading.value = false;
  }
};

const copyEntry = async (path: string) => {
  try {
    // We can't use read_remote_file directly as it's meant for SFTP, 
    // let's assume we can read local file via backend or just use a generic read
    const content = await invoke<string>('read_remote_file', { remotePath: path });
    await navigator.clipboard.writeText(content);
    alert("Copied to clipboard!");
  } catch (e) {
    alert("Copy failed: " + e);
  }
};

onMounted(loadVault);
</script>

<template>
  <div class="vault-view">
    <header class="vault-header">
      <span>GHOST_VAULT</span>
      <button class="refresh-btn" @click="loadVault" :disabled="isLoading">RESCAN</button>
    </header>
    
    <div class="vault-list scroller">
      <div v-for="entry in vaultEntries" :key="entry.name" class="vault-card">
        <div class="vault-info">
          <div class="vault-name">{{ entry.name }}</div>
          <div class="vault-meta">ARCHIVE_MD</div>
        </div>
        <button class="copy-icon-btn" @click="copyEntry(entry.path)" title="Copy to Clipboard">
          📋
        </button>
      </div>
      <div v-if="vaultEntries.length === 0" class="empty-hint">No archives captured yet.</div>
    </div>
  </div>
</template>

<style scoped>
.vault-view { display: flex; flex-direction: column; height: 100%; background: #000; }
.vault-header { padding: 10px 16px; border-bottom: 1px solid #18181b; display: flex; justify-content: space-between; align-items: center; font-size: 10px; color: #22c55e; letter-spacing: 2px; }
.refresh-btn { background: transparent; border: 1px solid #27272a; color: #71717a; font-size: 9px; cursor: pointer; padding: 2px 6px; border-radius: 4px; }
.refresh-btn:hover { color: #fff; border-color: #3b82f6; }

.vault-list { flex: 1; overflow-y: auto; padding: 10px; }
.vault-card { background: rgba(34, 197, 94, 0.03); border: 1px solid #18181b; border-radius: 6px; padding: 10px; margin-bottom: 8px; display: flex; justify-content: space-between; align-items: center; transition: all 0.2s; }
.vault-card:hover { border-color: #22c55e; box-shadow: 0 0 10px rgba(34, 197, 94, 0.1); }

.vault-info { flex: 1; min-width: 0; }
.vault-name { font-size: 11px; color: #d4d4d8; font-family: 'JetBrains Mono', monospace; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.vault-meta { font-size: 9px; color: #52525b; margin-top: 4px; }

.copy-icon-btn { background: transparent; border: none; cursor: pointer; font-size: 14px; opacity: 0.6; }
.copy-icon-btn:hover { opacity: 1; transform: scale(1.1); }

.empty-hint { text-align: center; font-size: 10px; color: #3f3f46; padding: 20px; }
</style>
