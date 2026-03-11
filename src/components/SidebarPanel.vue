<script setup lang="ts">
const props = defineProps<{
  files: any[];
  bgTabs: any[];
  skills: any[];
  cpuChartRef: any;
  memChartRef: any;
  isAutoPilot: boolean;
}>();

const emit = defineEmits(['switch-tab', 'proc-context', 'update:isAutoPilot', 'audit-ui', 'switch-mode', 'run-skill', 'change-dir']);

const updateAutoPilot = (e: any) => {
  emit('update:isAutoPilot', e.target.checked);
};

const onItemClick = (f: any) => {
  if (f.is_dir) {
    emit('change-dir', f.name);
  }
};
</script>

<template>
  <aside class="side-bar">
    <div class="module sys-health" @click="$emit('switch-mode', 0)" style="cursor: pointer;">
      <header>System Health (Dashboard)</header>
      <div class="chart-box">
        <div :ref="cpuChartRef" class="mini-chart"></div>
        <div :ref="memChartRef" class="mini-chart"></div>
      </div>
    </div>

    <div class="module scroller processes">
      <header>Running Processes</header>
      <ul class="data-list">
        <li v-for="t in bgTabs" :key="t.id" @click="$emit('switch-tab', t.id)" @contextmenu.prevent="$emit('proc-context', {event: $event, tab: t})">
          <span class="icon">📟</span>
          <span class="name">{{ t.title }}</span>
          <span class="val active">ACTIVE</span>
        </li>
        <li v-if="bgTabs.length === 0" class="empty-hint">No background tasks</li>
      </ul>
    </div>

    <div class="module scroller skills-hub">
      <header>Skill Hub (Remote)</header>
      <ul class="data-list">
        <li v-for="s in skills" :key="s.id" @click="$emit('run-skill', s.rpc)" :title="s.description">
          <span class="icon">{{ s.icon || '🛠️' }}</span>
          <span class="name">{{ s.name }}</span>
          <span class="val">RUN</span>
        </li>
        <li v-if="skills.length === 0" class="empty-hint">No skills in .ter/skills.json</li>
      </ul>
    </div>

    <div class="module scroller explorer">
      <header>SFTP Explorer</header>
      <ul class="data-list">
        <li @click="$emit('change-dir', '..')">
          <span class="icon">📁</span>
          <span class="name">..</span>
        </li>
        <li v-for="f in files" :key="f.name" @click="onItemClick(f)">
          <span class="icon">{{ f.is_dir ? '📁' : '📄' }}</span>
          <span class="name">{{ f.name }}</span>
        </li>
      </ul>
    </div>

    <div class="sidebar-footer">
      <header>AI Control</header>
      <div class="ai-controls">
        <button @click="$emit('switch-mode', 1)" class="btn-cyber">🌐 Open Cyber View</button>
        <button @click="$emit('audit-ui')" class="btn-audit">📸 Audit UI</button>
        <div class="toggle-box">
          <span>Auto-Pilot</span>
          <input type="checkbox" :checked="isAutoPilot" @change="updateAutoPilot" id="at-sidebar" />
          <label for="at-sidebar" class="switch"></label>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.side-bar { background: #0a192f; width: 260px; height: 100%; display: flex; flex-direction: column; flex-shrink: 0; border-right: 1px solid #1a1a1c; }
.module { padding: 15px; border-bottom: 1px solid #1a1a1c; }
.module header { font-size: 10px; text-transform: uppercase; color: #52525b; margin-bottom: 12px; font-weight: bold; }
.scroller { flex: 1; overflow-y: auto; }
.chart-box { display: flex; gap: 10px; height: 50px; }
.mini-chart { flex: 1; height: 100%; background: #161618; border-radius: 4px; }

.data-list { list-style: none; padding: 0; margin: 0; }
.data-list li { display: flex; justify-content: space-between; padding: 6px 8px; font-size: 11px; border-radius: 4px; color: #a1a1aa; cursor: pointer; border-bottom: 1px solid rgba(255,255,255,0.03); }
.data-list li:hover { background: #1a1a1c; color: #fff; }
.data-list .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 140px; }
.data-list .val.active { color: #22c55e; font-weight: bold; }
.data-list .val { color: #6366f1; font-size: 9px; border: 1px solid #6366f1; padding: 1px 4px; border-radius: 3px; }

.empty-hint { font-size: 10px; color: #3f3f46; text-align: center; margin-top: 20px; }

.skills-hub { max-height: 200px; flex: 0 0 auto; }

.sidebar-footer { padding: 15px; background: #080809; border-top: 1px solid #1a1a1c; margin-top: auto; }
.ai-controls { display: flex; flex-direction: column; gap: 8px; }
.btn-audit { background: #6366f1; border: none; color: white; padding: 6px; border-radius: 4px; cursor: pointer; font-size: 11px; font-weight: bold; }
.btn-cyber { background: #1a1a1c; border: 1px solid #3f3f46; color: #d4d4d8; padding: 6px; border-radius: 4px; cursor: pointer; font-size: 11px; font-weight: bold; }
.btn-cyber:hover { border-color: #6366f1; color: #fff; }
.toggle-box { display: flex; justify-content: space-between; align-items: center; font-size: 10px; color: #71717a; }

.switch { position: relative; display: inline-block; width: 30px; height: 16px; background: #333; border-radius: 10px; cursor: pointer; }
.switch::after { content: ''; position: absolute; width: 12px; height: 12px; background: #fff; border-radius: 50%; top: 2px; left: 2px; transition: 0.2s; }
input[type="checkbox"] { display: none; }
input:checked + .switch { background-color: #6366f1; }
input:checked + .switch::after { left: 16px; }
</style>
