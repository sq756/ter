<script setup lang="ts">
import { ref, onMounted } from 'vue';

const props = defineProps<{
  isOpen: boolean;
}>();

const emit = defineEmits(['close', 'update-macros']);

const macros = ref<{name: string, cmd: string}[]>([]);

onMounted(() => {
  const saved = localStorage.getItem('ter_macros');
  if (saved) {
    macros.value = JSON.parse(saved);
  } else {
    // Default Presets
    macros.value = [
      { name: 'Root', cmd: 'sudo su -' },
      { name: 'Exit', cmd: 'exit' },
      { name: 'Status', cmd: 'top' },
      { name: 'Clear', cmd: 'clear' }
    ];
    saveMacros();
  }
});

const saveMacros = () => {
  localStorage.setItem('ter_macros', JSON.stringify(macros.value));
  emit('update-macros', macros.value);
};

const addMacro = () => {
  macros.value.push({ name: 'New Macro', cmd: '' });
  saveMacros();
};

const removeMacro = (index: number) => {
  macros.value.splice(index, 1);
  saveMacros();
};
</script>

<template>
  <Transition name="slide">
    <div v-if="isOpen" class="settings-drawer">
      <header class="drawer-header">
        <div class="title-group">
          <span class="icon">⚙️</span>
          <h3>SYSTEM_SETTINGS</h3>
        </div>
        <button class="close-btn" @click="$emit('close')">×</button>
      </header>

      <div class="drawer-content">
        <section class="config-section">
          <header>⌨️ QUICK MACROS (Morse Light Right-Click)</header>
          <div class="macro-list">
            <div v-for="(m, i) in macros" :key="i" class="macro-item">
              <input v-model="m.name" @change="saveMacros" class="macro-name" placeholder="Label" />
              <input v-model="m.cmd" @change="saveMacros" class="macro-cmd" placeholder="Command" />
              <button @click="removeMacro(i)" class="delete-btn">🗑️</button>
            </div>
          </div>
          <button @click="addMacro" class="btn-add-macro">+ Add New Macro</button>
        </section>

        <section class="config-section disabled">
          <header>🎨 UI PREFERENCES (Coming Soon)</header>
          <p class="hint">Themes, Glow Intensity, and Light Colors.</p>
        </section>
      </div>

      <footer class="drawer-footer">
        <span class="ver">TER_CORE v2.2.14</span>
        <span class="status">ENCRYPTION_ACTIVE</span>
      </footer>
    </div>
  </Transition>
  <div v-if="isOpen" class="drawer-overlay" @click="$emit('close')"></div>
</template>

<style scoped>
.settings-drawer {
  position: fixed;
  top: 0;
  right: 0;
  width: 380px;
  height: 100vh;
  background: rgba(9, 9, 11, 0.95);
  backdrop-filter: blur(20px);
  border-left: 1px solid #22c55e44;
  z-index: 1000000;
  display: flex;
  flex-direction: column;
  box-shadow: -10px 0 30px rgba(0, 0, 0, 0.5);
}

.drawer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  z-index: 999999;
}

.drawer-header {
  padding: 20px;
  border-bottom: 1px solid #27272a;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title-group { display: flex; align-items: center; gap: 12px; }
.title-group h3 { font-size: 14px; color: #22c55e; letter-spacing: 0.1em; margin: 0; }

.close-btn { background: transparent; border: none; color: #71717a; font-size: 24px; cursor: pointer; transition: color 0.2s; }
.close-btn:hover { color: #fff; }

.drawer-content { flex: 1; overflow-y: auto; padding: 20px; }

.config-section { margin-bottom: 30px; }
.config-section header { font-size: 11px; color: #71717a; margin-bottom: 15px; font-weight: bold; }

.macro-list { display: flex; flex-direction: column; gap: 10px; margin-bottom: 15px; }
.macro-item { display: flex; gap: 8px; align-items: center; background: rgba(255, 255, 255, 0.03); padding: 8px; border-radius: 6px; border: 1px solid #27272a; }

.macro-name { width: 80px; background: transparent; border: none; border-bottom: 1px dashed #3f3f46; color: #fff; font-size: 12px; outline: none; }
.macro-cmd { flex: 1; background: transparent; border: none; color: #22c55e; font-size: 12px; font-family: monospace; outline: none; }

.delete-btn { background: transparent; border: none; cursor: pointer; opacity: 0.4; transition: opacity 0.2s; }
.delete-btn:hover { opacity: 1; }

.btn-add-macro { width: 100%; padding: 8px; background: rgba(34, 197, 94, 0.1); border: 1px dashed #22c55e; color: #22c55e; border-radius: 6px; cursor: pointer; font-size: 11px; transition: all 0.2s; }
.btn-add-macro:hover { background: rgba(34, 197, 94, 0.2); }

.config-section.disabled { opacity: 0.4; }
.hint { font-size: 10px; color: #52525b; font-style: italic; }

.drawer-footer { padding: 15px 20px; border-top: 1px solid #27272a; display: flex; justify-content: space-between; align-items: center; font-family: monospace; font-size: 10px; color: #3f3f46; }

.slide-enter-active, .slide-leave-active { transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1); }
.slide-enter-from, .slide-leave-to { transform: translateX(100%); }
</style>