<script setup lang="ts">
import { ref, onMounted } from 'vue';

const props = defineProps<{
  isOpen: boolean;
  useNativeWebview: boolean;
  isSafeMode: boolean;
  sidebarSlots: string[];
}>();

const emit = defineEmits(['close', 'update-macros', 'update:useNativeWebview', 'update:isSafeMode', 'update:sidebarSlots']);

const allViews = ['OPS', 'ARS', 'NAV', 'LOGS'];

const toggleSlot = (view: string) => {
  const current = [...props.sidebarSlots];
  if (current.includes(view)) {
    if (current.length > 1) {
      const idx = current.indexOf(view);
      current.splice(idx, 1);
    }
  } else {
    if (current.length < 3) {
      current.push(view);
    } else {
      current.shift();
      current.push(view);
    }
  }
  emit('update:sidebarSlots', current);
};

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
          <header>📐 SIDEBAR_DECK SLOTS (Select 3)</header>
          <div class="slot-selector">
            <button v-for="v in allViews" :key="v" 
                    :class="{ active: sidebarSlots.includes(v) }"
                    @click="toggleSlot(v)">
              {{ v }}
            </button>
          </div>
          <p class="hint">The 3rd slot is dynamic and can be overridden by AGENT protocols.</p>
        </section>

        <section class="config-section">
          <header>🛡️ SYSTEM_STABILITY (Emergency Rescue)</header>
          <div class="setting-row">
            <span class="label">SAFE_MODE (Disable Webviews & Glow)</span>
            <label class="mini-switch">
              <input type="checkbox" :checked="isSafeMode" @change="$emit('update:isSafeMode', $event.target.checked)" />
              <span class="slider"></span>
            </label>
          </div>
          <p class="hint">Enable this if you experience black screens or UI lag.</p>
        </section>

        <section class="config-section">
          <header>🌐 WEB_ENGINE CONFIG (Native UI Mode)</header>
          <div class="setting-row">
            <span class="label">USE_NATIVE_ENGINE (Unlock Google/GitHub)</span>
            <label class="mini-switch">
              <input type="checkbox" :checked="useNativeWebview" @change="$emit('update:useNativeWebview', $event.target.checked)" />
              <span class="slider"></span>
            </label>
          </div>
          <p class="hint">Toggle this if you see login restrictions in Webview.</p>
        </section>

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
  z-index: 2147483647;
  display: flex;
  flex-direction: column;
  box-shadow: -10px 0 30px rgba(0, 0, 0, 0.5);
}

.drawer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  z-index: 2147483646;
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

.setting-row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 15px; background: rgba(255, 255, 255, 0.03); padding: 10px; border-radius: 6px; border: 1px solid #27272a; }

.slot-selector { display: flex; gap: 8px; margin-bottom: 10px; }
.slot-selector button { flex: 1; padding: 6px; background: #000; border: 1px solid #27272a; color: #52525b; font-size: 10px; font-family: 'JetBrains Mono', monospace; cursor: pointer; border-radius: 4px; transition: all 0.2s; }
.slot-selector button.active { border-color: #22c55e; color: #22c55e; box-shadow: 0 0 10px rgba(34, 197, 94, 0.2); }

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