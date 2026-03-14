<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  id: string;
  path: string;
  initialContent: string;
}>();

const emit = defineEmits(['save-complete']);

const content = ref(props.initialContent);
const isDirty = ref(false);
const isSaving = ref(false);

watch(content, (newVal) => {
  if (newVal !== props.initialContent) {
    isDirty.value = true;
  } else {
    isDirty.value = false;
  }
});

const handleSave = async () => {
  if (isSaving.value) return;
  isSaving.value = true;
  try {
    await invoke('write_remote_file', { remotePath: props.path, content: content.value });
    isDirty.value = false;
    emit('save-complete');
    console.log("[TerEditor] File saved successfully");
  } catch (e) {
    alert("Save failed: " + e);
  } finally {
    isSaving.value = false;
  }
};

const handleKeyDown = (e: KeyboardEvent) => {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault();
    handleSave();
  }
};

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<template>
  <div class="ter-editor">
    <header class="editor-toolbar">
      <span class="file-path">{{ path }}</span>
      <div class="toolbar-actions">
        <span v-if="isDirty" class="dirty-indicator">MODIFIED</span>
        <button class="save-btn" :disabled="!isDirty || isSaving" @click="handleSave">
          {{ isSaving ? 'SAVING...' : 'SAVE [CTRL+S]' }}
        </button>
      </div>
    </header>
    <textarea v-model="content" class="editor-area" spellcheck="false"></textarea>
  </div>
</template>

<style scoped>
.ter-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background: #000;
}

.editor-toolbar {
  height: 28px;
  background: #18181b;
  border-bottom: 1px solid #27272a;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 12px;
  flex-shrink: 0;
}

.file-path {
  font-size: 10px;
  color: #71717a;
  font-family: 'JetBrains Mono', monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 70%;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.dirty-indicator {
  font-size: 9px;
  color: #eab308;
  font-weight: bold;
}

.save-btn {
  background: #27272a;
  border: 1px solid #3f3f46;
  color: #fff;
  font-size: 9px;
  padding: 2px 8px;
  border-radius: 2px;
  cursor: pointer;
  transition: all 0.1s;
}

.save-btn:hover:not(:disabled) {
  background: #22c55e;
  color: #000;
  border-color: #22c55e;
}

.save-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.editor-area {
  flex: 1;
  background: #000;
  color: #d4d4d8;
  border: none;
  resize: none;
  padding: 15px;
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 13px;
  line-height: 1.6;
  outline: none;
}
</style>
