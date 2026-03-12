import { ref, computed, type Ref } from 'vue';
import * as echarts from 'echarts';

export function useStats(currentAgentPort: Ref<number | null>, agentToken: Ref<string>) {
  const cpuChartRef = ref<HTMLElement | null>(null);
  const memChartRef = ref<HTMLElement | null>(null);
  let cpuChart: any, memChart: any;
  const cpuHistory = ref<number[]>([]);
  const memHistory = ref<number[]>([]);

  const currentCpuUsage = computed(() => 
    cpuHistory.value.length > 0 ? cpuHistory.value[cpuHistory.value.length - 1] : 0
  );

  const getChartOpt = (d: any[], c: string) => ({ 
    grid: { top: 5, bottom: 0, left: 0, right: 0 }, 
    xAxis: { type: 'category', show: false }, 
    yAxis: { type: 'value', min: 0, max: 100, show: false }, 
    series: [{ 
      data: d, 
      type: 'line', 
      smooth: true, 
      areaStyle: { color: c }, 
      itemStyle: { color: c }, 
      showSymbol: false 
    }], 
    animation: false 
  });

  const initCharts = () => { 
    if (cpuChartRef.value) cpuChart = echarts.init(cpuChartRef.value); 
    if (memChartRef.value) memChart = echarts.init(memChartRef.value); 
  };

  const fetchStats = async () => {
    if (!currentAgentPort.value) return;
    try {
      const r = await fetch(`http://localhost:${currentAgentPort.value}/stats`, { 
        headers: { 'X-Ter-Token': agentToken.value } 
      });
      const d = await r.json();
      cpuHistory.value.push(d.cpu_usage); 
      memHistory.value.push((d.mem_used / d.mem_total) * 100);
      
      if (cpuHistory.value.length > 30) { 
        cpuHistory.value.shift(); 
        memHistory.value.shift(); 
      }
      
      cpuChart?.setOption(getChartOpt(cpuHistory.value, '#6366f1')); 
      memChart?.setOption(getChartOpt(memHistory.value, '#a855f7'));
    } catch (e) {
      console.warn("Stats fetch failed", e);
    }
  };

  return {
    cpuChartRef,
    memChartRef,
    currentCpuUsage,
    initCharts,
    fetchStats
  };
}
