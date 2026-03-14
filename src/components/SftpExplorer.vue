<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  currentPath: string;
  files: any[];
}>();

const emit = defineEmits(['change-dir', 'item-context', 'item-drag-start']);

const breadcrumbs = computed(() => {
  const parts = props.currentPath.split('/').filter(p => p);
  const result = [{ name: 'ROOT', path: '/' }];
  let current = '';
  for (const part of parts) {
    current += '/' + part;
    result.push({ name: part.toUpperCase(), path: current });
  }
  return result;
});

const sortedFiles = computed(() => {
  const baseFiles = props.files.filter(f => f.name !== '..');
  return [...baseFiles].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });
});

const onItemClick = (f: any) => {
  if (f.is_dir) emit('change-dir', f.name);
};
</script>

<template>
  <div class="sftp-explorer-container">
    <header class="explorer-header">
      <span class="title">SFTP_EXPLORER</span>
      <div class="breadcrumbs">
        <template v-for="(bc, i) in breadcrumbs" :key="bc.path">
          <span class="bc-item" @click="$emit('change-dir', bc.path)">{{ bc.name }}</span>
          <span v-if="i < breadcrumbs.length - 1" class="bc-sep">></span>
        </template>
      </div>
    </header>

    <div class="explorer-body scroller-enhanced">
      <ul class="file-list">
        <!-- Parent Dir (v2.11.52 Boundary Protection) -->
        <li class="file-item" 
            :class="{ 'disabled': currentPath === '/' }"
            @click="currentPath !== '/' && $emit('change-dir', '..')" 
            @contextmenu.prevent="currentPath !== '/' && $emit('item-context', { event: $event, file: { name: '..', is_dir: true } })">
          <span class="icon">⤴️</span>
          <span class="name">..</span>
        </li>
        <!-- Files & Folders -->
        <li v-for="f in sortedFiles" :key="f.name" 
            class="file-item" 
            @click="onItemClick(f)"
            @contextmenu.prevent="$emit('item-context', { event: $event, file: f })"
            draggable="true"
            @dragstart="$emit('item-drag-start', f)">
          <span class="icon">{{ f.is_dir ? '📂' : '📄' }}</span>
          <span class="name" :title="f.path">{{ f.name }}</span>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.sftp-explorer-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: transparent;
}

.explorer-header {
  padding: 12px 16px;
  border-bottom: 1px solid #27272a;
  flex-shrink: 0;
}

.explorer-header .title {
  font-size: 10px;
  color: #71717a;
  letter-spacing: 2px;
  display: block;
  margin-bottom: 8px;
}

.breadcrumbs {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
}

.bc-item {
  color: #22c55e;
  cursor: pointer;
  transition: all 0.1s;
}

.bc-item:hover {
  text-decoration: underline;
  filter: brightness(1.2);
}

.bc-sep {
  color: #3f3f46;
  user-select: none;
}

.explorer-body {
  flex: 1;
  overflow-y: auto !important;
  min-height: 0;
}

.scroller-enhanced::-webkit-scrollbar { width: 4px; }
.scroller-enhanced::-webkit-scrollbar-thumb { background: #27272a; border-radius: 2px; }

.file-list {
  list-style: none;
  padding: 8px;
  margin: 0;
}

.file-item {
  display: flex;
  align-items: center;
  padding: 6px 10px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.1s;
  font-size: 13px;
  color: #d4d4d8;
  margin-bottom: 1px;
}

.file-item.disabled {
  opacity: 0.3;
  cursor: not-allowed;
  pointer-events: none;
}

.file-item:hover {
  background: rgba(34, 197, 94, 0.08);
  color: #22c55e;
}

.file-item .icon {
  margin-right: 8px; /* Fixed 8px spacing */
  font-size: 14px;
  flex-shrink: 0;
}

.file-item .name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis; /* Automatic truncation */
  flex: 1;
}
</style>
