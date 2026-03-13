import { ref, computed, type Ref } from 'vue';
import * as echarts from 'echarts';

export type HealthMode = 'resource' | 'network' | 'detail';

export function useStats(currentAgentPort: Ref<number | null>, agentToken: Ref<string>) {
  const cpuChartRef = ref<HTMLElement | null>(null);
  const memChartRef = ref<HTMLElement | null>(null);
  const netChartRef = ref<HTMLElement | null>(null);
  
  let cpuChart: any, memChart: any, netChart: any;
  
  const cpuHistory = ref<number[]>([]);
  const memHistory = ref<number[]>([]);
  const netHistory = ref<number[]>([]);
  
  const healthMode = ref<HealthMode>('resource');
  const lastNetStats = ref({ sent: 0, recv: 0, time: Date.now() });
  const currentNetSpeed = ref({ up: '0 B/s', down: '0 B/s' });
  const extraStats = ref<any>({ 
    gpu: 'N/A', 
    uptime: '0s', 
    ip: '127.0.0.1',
    disk: '0/0 GB'
  });

  const currentCpuUsage = computed(() => 
    cpuHistory.value.length > 0 ? cpuHistory.value[cpuHistory.value.length - 1] : 0
  );

  const formatSpeed = (bytes: number) => {
    if (bytes < 1024) return `${bytes.toFixed(1)} B/s`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB/s`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB/s`;
  };

  const getChartOpt = (d: any[], c: string, max = 100) => ({ 
    grid: { top: 5, bottom: 0, left: 0, right: 0 }, 
    xAxis: { type: 'category', show: false }, 
    yAxis: { type: 'value', min: 0, max, show: false }, 
    series: [{ 
      data: d, 
      type: 'line', 
      smooth: true, 
      areaStyle: { color: c, opacity: 0.2 }, 
      itemStyle: { color: c }, 
      showSymbol: false,
      lineStyle: { width: 1.5 }
    }], 
    animation: false 
  });

  const initCharts = () => { 
    if (cpuChartRef.value) cpuChart = echarts.init(cpuChartRef.value); 
    if (memChartRef.value) memChart = echarts.init(memChartRef.value); 
    if (netChartRef.value) netChart = echarts.init(netChartRef.value);
  };

  const fetchStats = async () => {
    if (!currentAgentPort.value) return;
    try {
      const r = await fetch(`http://localhost:${currentAgentPort.value}/stats`, { 
        headers: { 'X-Ter-Token': agentToken.value } 
      });
      if (!r.ok) return;
      const d = await r.json();
      
      // Update Histories
      cpuHistory.value.push(d.cpu_usage); 
      memHistory.value.push((d.mem_used / d.mem_total) * 100);
      
      // Calc Network Speed
      const now = Date.now();
      const dt = (now - lastNetStats.value.time) / 1000;
      if (dt > 0) {
        const up = (d.net_sent - lastNetStats.value.sent) / dt;
        const down = (d.net_recv - lastNetStats.value.recv) / dt;
        currentNetSpeed.value = { up: formatSpeed(up), down: formatSpeed(down) };
        netHistory.value.push(up + down);
        lastNetStats.value = { sent: d.net_sent, recv: d.net_recv, time: now };
      }

      if (cpuHistory.value.length > 30) { 
        cpuHistory.value.shift(); 
        memHistory.value.shift(); 
        netHistory.value.shift();
      }
      
      // Update Charts based on mode
      if (healthMode.value === 'resource') {
        cpuChart?.setOption(getChartOpt(cpuHistory.value, '#22c55e')); 
        memChart?.setOption(getChartOpt(memHistory.value, '#3b82f6'));
      } else if (healthMode.value === 'network') {
        netChart?.setOption(getChartOpt(netHistory.value, '#a855f7', Math.max(...netHistory.value, 1024)));
      }

      // Update Extra Info
      extraStats.value = {
        gpu: d.gpu_info || 'N/A',
        uptime: Math.floor(d.uptime) + 's',
        ip: d.ip || '127.0.0.1',
        disk: `${(d.disk_used / 1024 / 1024 / 1024).toFixed(1)}/${(d.disk_total / 1024 / 1024 / 1024).toFixed(1)} GB`
      };
      
    } catch (e) {
      console.warn("Stats fetch failed", e);
    }
  };

  const setHealthMode = (mode: HealthMode) => {
    healthMode.value = mode;
    // Force re-init or clear if needed
    setTimeout(initCharts, 50);
  };

  return {
    cpuChartRef,
    memChartRef,
    netChartRef,
    currentCpuUsage,
    healthMode,
    currentNetSpeed,
    extraStats,
    initCharts,
    fetchStats,
    setHealthMode
  };
}
