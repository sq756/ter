# Ter (⚡) - AI-Native Visual SSH Platform

**“为人设计 UI，为 AI 设计接口”**

Ter 是一个基于 Tauri v2、Vue 3 和 Rust 构建的高性能 SSH 可视化工作站。它超越了传统的终端工具，旨在成为 AI 智能体（如 Gemini CLI）操作远程服务器的“物理身体”。

![Version](https://img.shields.io/badge/version-0.2.2-blueviolet)
![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)

---

## 🌟 核心特性 (v0.2.2+)

### 👁️ 视觉闭环 (Visual Audit Loop)
*   **AI 的眼睛**：集成 Headless Chrome，支持通过控制 API 截取实时 UI 画面。
*   **SFTP 顺传**：截取的 UI 快照会通过 SSH 隧道秒传至远程 Linux 的 `/tmp/current_ui.png`，供 AI 进行视觉校验。
*   **全自动截图**：手动点击“Audit UI”或通过代码逻辑自动触发视觉验收。

### 🤖 自动驾驶模式 (Auto-Pilot)
*   **RPC 拦截器**：内置终端流拦截器，自动识别 `[TER_RPC]` 指令。
*   **无人值守开发**：开启 Auto-Pilot 后，AI 可以自主请求截图、获取系统状态并根据反馈修正代码，实现“修改-编译-截图-分析”的全自动闭环。

### 🧩 插件生态系统 (Plugin Hub)
*   **UI 注入 (Flow B)**：支持第三方插件通过标准 JSON 输出动态注入 Vue 组件。
*   **语言无关**：任何人都可以用 Python/Node/Go 编写插件，只需输出特定的 JSON 结构（Text, Chart, Form 等）。
*   **MCP 思想**：兼容 Model Context Protocol 理念，让 AI 零成本理解环境技能。

### 🔒 安全与基础
*   **Secure Vault**：主密码保护的服务器凭据加密存储 (AES-256-GCM)。
*   **Cyber Mode**：独特的磨砂玻璃半透明布局，实时监控后端日志流。
*   **Go Agent**：极致轻量（<10MB）的服务器监控程序，支持进程托管与任务持久化。

---

## 🛠️ AI 交互协议 ([TER_RPC])

你可以直接在终端中输入（或让 AI 输出）以下暗号来触发系统动作：

| 指令 (JSON) | 动作 | 效果 |
| :--- | :--- | :--- |
| `[TER_RPC] {"action": "screenshot"}` | **触发视觉捕捉** | 截图并上传至 Linux 端的 `/tmp/current_ui.png` |
| `[TER_RPC] {"action": "notify", "msg": "..."}` | **系统通知** | 在 Ter 界面右侧弹出磨砂玻璃通知 |

---

## 🧩 开发者：如何编写插件？

只需在 `~/.ter/plugins/你的插件名/` 下创建一个 `manifest.yaml`：

```yaml
name: "My Analytics"
description: "分析服务器日志并绘图"
command: "python main.py"
parameters: []
```

让你的脚本输出 JSON 即可实现 UI 注入：
```bash
# 输出一个 ECharts 条形图
echo '{"type": "chart", "message": [10, 20, 30, 40]}'
```

---

## 🚀 快速开始

1. **解压运行**：从 Releases 下载对应平台的安装包。
2. **解锁保险库**：首次运行设置主密码。
3. **添加服务器**：保存 SSH 配置，Ter 会自动为你下发并启动 Go Agent。
4. **开启 AI 审计**：在侧边栏底部开启 **Auto-Pilot**，开始享受 AI 原生开发的快感。

---

## 🏗️ 技术架构
*   **前端**: Vue 3 + Vite + ECharts + xterm.js + html2canvas
*   **后端**: Rust (Tauri v2) + russh (SSH2) + tiny_http (AI Trigger API)
*   **Agent**: Go (Static Compilation)
*   **AI 控制面**: 监听 `127.0.0.1:1414` 提供外部抓拍快门。

---

Built with ❤️ by [sq756](https://github.com/sq756)
