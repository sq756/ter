# Ter (⚡) Development Log & Memory Core
**Last Update:** March 15, 2026 (v2.14.18)

## 1. Project Status Summary
Ter has evolved from a simple SSH client into an **AI-Native Visual SSH Workstation**. The current architecture (v2.14.x) utilizes a **Matrix Allocator** system for dynamic tiling and a centralized **TER_CORE State Center**.

## 2. Solved Issues (v2.14.10 -> v2.14.18)
- **UI & Layout**:
    - **Flexible Sidebar**: Removed fixed 260px width; `SIDEBAR_PANEL` now correctly follows Workspace Matrix ratios.
    - **Tab-Bar Overhaul**: Redesigned top bar with non-blocking drag regions (`drag-spacer`) and scrollable tab area.
    - **Status Bar Cleanup**: Removed emoji icons, unified fonts to `JetBrains Mono`, and fixed `agent-zone` cursor pointers.
- **Terminal Interaction**:
    - **Focus Recovery**: Implemented explicit focus capture on Tab switch and Pane click. Fixed Windows focus-loss bug.
    - **Lag Mitigation**: Added 50ms debounce to `performFit` and 16ms pulse aggregation for PTY data.
    - **Jump to Bottom**: Added a smart floating button that appears during scroll-back and auto-hides in full-screen apps (vim/htop).
- **Custom Features**:
    - **Cyber Cursor**: Implemented a customizable breathing dot cursor with a glow effect.
    - **Cursor Bug**: Fixed the "ghosting/jumping" issue caused by conflicting CSS keyframe transforms.
- **Stability**:
    - **Event Tunneling**: Fixed broken event chains by explicitly forwarding emits through `MatrixAllocator` and `TileContainer`.
    - **Script Crash**: Resolved a fatal crash in `SidebarPanel` caused by accessing undefined `props.files`.

## 3. Core Architecture Guide (Quick Start)
- **Entry Point**: `src/App.vue` - Orchestrates the entire shell, modals, and context menus.
- **State Management**: `src/store.ts` - Contains `globalState` (reactive) and `storeActions`.
- **Layout Engine**: `src/components/MatrixAllocator.vue` -> `TileContainer.vue` -> `WidgetRegistry.ts`.
- **Terminal Logic**: `src/components/TerminalView.vue` (UI) and `src/TerminalManager.ts` (xterm.js instances).
- **Backend**: `src-tauri/src/lib.rs` - Rust PTY handling, SSH session management, and SQLite storage.

## 4. Known Challenges & Pending Tasks
- **Challenge**: `data-tauri-drag-region` on Windows is aggressive and can block clicks if not precisely placed.
- **Pending**:
    - **Local Terminal**: Integration of native shells (PowerShell/Zsh) to make Ter a full "Local-First" terminal.
    - **Auth Trigger**: Enabling the `ter` command in local shell to trigger the Master Password unlock.
    - **Cursor Expansion**: Support for custom SVG/Pixel-art cursor imports.
    - **State Migrator**: A robust system to handle LocalStorage schema updates without clearing user data.

## 5. Lessons Learned & Error Experience
- **Vue 3 Nesting**: When nesting components >2 levels deep (App -> Allocator -> Container -> Plugin), **Explicit Event Tunneling** is safer than relying on `v-on="$attrs"`.
- **Animation Conflict**: NEVER use `transform` in CSS `@keyframes` if the element's position is also being updated via JavaScript `transform` styles. Use `opacity` or `filter` for breathing effects instead.
- **Tauri Drag Regions**: In Windows, a drag region element must be a sibling or a specific background layer to avoid "eating" clicks intended for foreground buttons.

## 6. v2.15 Roadmap (The "Local-First" Era)
1. Implement `spawn_local_pty` in Rust using `portable-pty`.
2. Update `CyberGate.vue` to allow "Guest Mode" (Local Shell only).
3. Implement the `[TER_AUTH]` PTY interceptor to trigger the Master Password vault mid-session.
