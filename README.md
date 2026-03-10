# Ter (⚡) - Modern Visual SSH Manager

**Ter** is a lightweight, secure, and visually rich SSH terminal and server management tool built with **Tauri**, **Vue 3**, and **Rust**. It goes beyond a simple terminal by providing real-time resource monitoring, file management, and an integrated AI sidekick.

![Version](https://img.shields.io/badge/version-2.1.1-blue.svg)
![License](https://img.shields.io/badge/license-MIT-green.svg)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)

## ✨ Features

- **🔒 Secure Vault**: Protect your server credentials with a Master Password (AES-256-GCM + Argon2).
- **📟 Integrated Terminal**: High-performance terminal based on `xterm.js` with WebGL acceleration.
- **📊 Real-time Monitoring**: Visual dashboards for CPU and Memory usage powered by `ECharts`.
- **📁 File Explorer**: Full-featured SFTP client for browsing, uploading, and downloading files.
- **✨ AI Sidekick**: Built-in AI (Web-LLM) to analyze terminal output, explain logs, and troubleshoot errors.
- **🛠️ Task Management**: Remote process monitoring and managed task control (start/stop/logs).
- **🖥️ Remote Desktop**: One-click VNC tunneling for GUI access to your remote servers.
- **📦 Lightweight Agent**: A Go-based agent automatically deployed to remote servers for advanced monitoring.

## 🚀 Quick Start

### Installation

Download the latest release for your platform from the [Releases](https://github.com/sq756/ter/releases) page.

- **Windows**: `.exe` or `.msi`
- **macOS**: `.dmg` (Universal)
- **Linux**: `.AppImage` or `.deb`

### Usage

1. **Unlock**: Set your master password on first launch.
2. **Add Server**: Click "+ Add" to save your server's SSH credentials.
3. **Connect**: Tap a server card to establish a secure connection.
4. **Explore**: Use the sidebar to switch between Terminal, File Manager, and AI Sidekick.

## 🛠️ Development

### Prerequisites

- **Rust**: [rustup.rs](https://rustup.rs/)
- **Node.js**: [nodejs.org](https://nodejs.org/) (LTS)
- **Go**: [go.dev](https://go.dev/) (For building the agent)

### Build from Source

1. **Clone the repository**:
   ```bash
   git clone https://github.com/sq756/ter.git
   cd ter
   ```

2. **Install dependencies**:
   ```bash
   npm install
   ```

3. **Build the Agent (Linux amd64)**:
   ```bash
   cd ter_agent
   GOOS=linux GOARCH=amd64 go build -o agent_linux_amd64 main.go
   cd ..
   ```

4. **Run in Development Mode**:
   ```bash
   npm run tauri dev
   ```

5. **Build for Production**:
   ```bash
   npm run tauri build
   ```

## 🏗️ Architecture

- **Frontend**: Vue 3 + TypeScript + Vite + ECharts + xterm.js
- **Backend**: Rust (Tauri v2) + SQLx (SQLite)
- **Agent**: Go (Single binary, minimal dependencies)
- **IPC**: Secure Tauri IPC for frontend-to-backend communication.
- **SSH/SFTP**: `russh` (Rust implementation of SSH-2).

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.

---
Built with ❤️ by [sq756](https://github.com/sq756)
