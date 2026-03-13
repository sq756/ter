# AI 原生远程工作站 (代号: Ter) 深度开发计划

## 0. 产品重新定位
**“不仅仅是 SSH，而是 AI 智能体的物理宿主与操作容器”**
Ter 旨在打破 AI 与远程服务器之间的次元壁。它通过加密的 SSH 隧道为 LLM 提供：
*   **眼睛**：集成视觉审计模块（Headless Chrome），让 AI 看见 UI。
*   **手**：PTY 终端与本地进程调度，让 AI 执行指令。
*   **无限扩展**：基于 MCP 思想的插件生态（Skill Protocol），连接万物。

---

## 1. 架构概览与技术栈

### 1.1 三层生态模型
```text
[ 第三方开源插件生态 (Ter Skills / Plugins) ] ---> 使用 Python/JS/Rust 编写
      | (通过 JSON-RPC / MCP 标准协议接入)
      v
[ 桌面端 (Tauri + Rust) : The Core ]  <========> [ Gemini CLI / 其他 LLM ]
      | (宿主环境、权限校验、视觉抓取、插件调度)          (决策、推理、代码生成)
      |
      +-- 前端 UI (Vue 3 / xterm.js / ECharts)
      +-- 底层通信 (SSH 隧道 / SFTP / 端口转发)
      |
[ 远程服务器 (Go Agent / 业务进程) ]
```

### 1.2 技术栈选型补全
| 模块 | 技术栈 | 核心职责 |
| :--- | :--- | :--- |
| **桌面端 Core** | Tauri + Rust (russh/tokio/sqlx) | 宿主环境管理、安全沙箱、AI 指令拦截与分发。 |
| **视觉审计** | headless_chrome (Rust) | 提供 `ai_audit_ui` API，生成 `snap/` 快照供 AI 校验。 |
| **插件引擎** | JSON-RPC over Stdio / WASI | 语言无关的插件运行环境，支持热插拔与沙箱安全。 |
| **桥接协议** | MCP (Model Context Protocol) | 让 AI 零成本理解环境能力（Skill Manifest）。 |
| **远程 Agent** | Go (Static Compilation) | 系统指标收集、进程管理、Local API。 |

---

## 2. 详细开发阶段

### 第一阶段：基础框架与终端互通 (Terminal & SSH) - [已完成]
*   **异步 SSH 引擎**: 基于 `russh` 实现 PTY 通道。
*   **前端集成**: xterm.js 流畅渲染与 WebGL 加速。
*   **基础存储**: SQLite 持久化服务器配置与 Master 密码加密。

### 第二阶段：自研守护进程 (Go Agent) - [进行中]
*   **轻量化采集**: CPU/内存/网络指标及 Top 10 进程快照。
*   **进程托管**: 类似 `screen/tmux` 的常驻任务管理与状态持久化。
*   **安全加固**: 仅监听 `127.0.0.1`，通过动态 Token 鉴权。

### 第三阶段：可视化穿透与视觉审计 (Vision & Dashboard) - [扩展进化]
*   **静默安装与隧道**: SFTP 自动分发 Agent，SSH 隧道打通 `54321` 本地端口映射。
*   **UI 影子渲染 (Shadow Renderer)**:
    *   集成 `headless_chrome`，实现 `ai_audit_ui` Tauri 指令。
    *   **视觉快照接口**: 自动创建 `snap/` 目录，保存带时间戳的 UI 快照，解决 AI 无法验证前端样式的问题。
*   **可视化监控**: 仪表盘展示资源利用率曲线与可交互进程管理器。

### 第四阶段：AI 枢纽与能力声明 (The AI Gateway) - [核心转折点]
*   **能力清单 (Skill Manifest)**:
    *   在项目根目录生成 `.ter/skills.json`，动态向 AI 声明当前可用的函数接口。
*   **指令拦截与调度**:
    *   Ter 捕获 AI 输出的特定指令（如截图、重启服务），在 Rust 后端执行而非直接运行 Bash。
*   **自动化自愈循环**:
    *   实现“代码修改 -> 触发编译 -> 捕获报错 -> 喂给 AI -> 再次编译 -> UI 截图验证”的闭环工作流。

### 第五阶段：开源插件机制与模块化生态 (The Plugin Ecosystem) - [全新战略]
*   **Ter Plugin API**: 采用 JSON-RPC 交互格式，插件只需包含 `manifest.yaml`（能力声明）和执行脚本。
*   **插件沙箱**:
    *   **低信任模块**: 运行在 WASM 沙箱中，限制文件系统访问。
    *   **高信任模块**: 运行本地 Python/Node 脚本，通过 Stdio 通信。
*   **包管理器 (Ter Hub)**:
    *   指令 `ter plugin install <url>`，一键下载并自动将新技能合并至 AI 的 Prompt 中。

---

## 3. 近期里程碑：v0.2.4 深度扩展预案

### A. 浏览器沙盒集成 (Cyber Webview)
*   **内置渲染容器**: 在右侧 Cyber 面板中嵌入真正的 Webview，支持实时渲染 AI 生成的网页代码。
*   **感知闭环**: Gemini CLI 修改代码后，Webview 自动刷新，配合 `ai_audit_ui` 实现真正的“所见即所得”自动化开发。

### B. 插件市场初探 (Skill Hub Entry)
*   **动态能力注入**: 利用 `list_plugins` 指令，允许用户通过 YAML 定义并上传自己的 AI 技能。
*   **MCP 协议深度适配**: 建立标准的技能描述规范，使 Ter 成为 AI 与本地工具之间的万能桥梁。

### C. 增强型视觉体验 (Hacker Vision)
*   **多维锁屏**: 除了 Matrix 瀑布流，引入 Three.js 打造 3D 服务器机房视角等增强型实时监控屏保。
*   **视觉自愈可视化**: 在执行视觉审计时，实时显示 AI 的“视觉分析”热力图。

---

## 4. 开源生态设计：别人如何为 Ter 写模块？

假设开发者想为 Ter 编写一个“arXiv 论文检索”模块：

**步骤 A：编写 `manifest.yaml` (供 AI 阅读)**
```yaml
name: arxiv_scholar
description: "搜索 arXiv 论文并获取 PDF 摘要，适用于 AI 进行学术调研。"
command: "python main.py"
parameters:
  - name: keyword
    type: string
    description: "要搜索的论文关键字"
```

**步骤 B：编写执行脚本 `main.py` (实际逻辑)**
```python
import sys, json
input_data = json.loads(sys.stdin.read()) # 接收 AI 传来的参数
# ... 执行请求 ...
result = {"papers": [{"title": "AI in SSH", "summary": "..."}]}
print(json.dumps(result)) # 返回结果给 Ter
```

**步骤 C：AI 的调用逻辑**
1. 用户要求 AI 调研论文。
2. AI 发现环境中有 `arxiv_scholar` 技能。
3. AI 发起调用请求。
4. Ter 拦截并运行脚本，将结果返回给 AI。

---

## 4. 总结
---

## 5. 安全与稳定性升级策略 (Safety & Stability Upgrade Strategy)

### 5.1 主线程死锁预防协议 (Deadlock Prevention Protocol)
*   **非阻塞原则**: 严禁在任何 `tauri::command` (Invoke Handler) 中使用 `std::sync::Mutex` 的 `lock()` 方法进行长时间等待。必须优先使用 `tokio::sync::Mutex` 或 `DashMap`。
*   **异步运行环境**: 所有涉及 SSH 通信、文件 I/O 或复杂计算的任务，必须通过 `tauri::async_runtime::spawn` 或 `tokio::spawn` 异步执行。
*   **初始化保护**: 严禁在 `setup` 阶段或 `main` 函数中进行阻塞式的网络探测或数据库大规模迁移。任何可能超过 50ms 的初始化任务必须异步化，并提供 UI 加载状态。

### 5.2 UI 演进兼容性准则 (UI Evolution Guidelines)
*   **Iframe 渲染基石**: 现有的 `<iframe>` 渲染模式作为 Cyber Webview 的稳定基准。在未通过 100% 兼容性测试前，严禁移除或默认替换为 Native Webview。
*   **渐进式增强**: 新的 Native 渲染能力应以“增强层”形式存在（如侧边栏预览或独立悬浮窗），通过功能开关（Feature Flag）控制，确保用户在 Native 环境失效时能快速回退至 Iframe。
*   **脚本安全性**: `AGENT_SCRIPT` 注入逻辑必须同时适配 Webview2 (Windows) 和 WebKit (Linux/macOS)，并处理好 Iframe 跨域限制下的降级方案。

### 5.3 状态管理守卫 (State Management Guardrails)
*   **主密码状态校验**: 所有敏感操作前，通过 `Option<Crypto>` 的非阻塞检查（如 `lock().await`）确认解锁状态。若未解锁，应返回标准的 `UNAUTHORIZED` 错误码，由前端统一引导至 CyberGate。
*   **资源自动回收**: 强化 PTY 和 SSH 通道的 Drop 机制。在 Tab 关闭或连接断开时，必须通过 `AbortHandle` 显式终止后台轮询任务，防止僵尸进程和内存泄漏。
