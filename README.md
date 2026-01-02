# Tether

**A minimalistic, black-hole themed file synchronization tool.**

Tether automatically mirrors your files to a backup drive instantly and securely. Designed for speed, privacy, and zero cognitive load.

## Features

- **Dark Mode First**: Deep `#050505` interface with traffic-light overlay.
- **Smart Sync**:
  - **Check & Resume**: Skips identical files to save time.
  - **Auto-Overwrite**: Updates modified files instantly.
- **Noise Filter**: Automatically ignores system junk (`.plist`, `.log`, `.DS_Store`, `node_modules`).
- **Safety**: Syncs to a dedicated `Tether_Backups` folder—never deletes your existing drive data.
- **Secure Audit Log**: Real-time activity feed of every file operation.

## Installation

### macOS
Download the latest `.dmg` from Releases.
Drag **Tether** to your Applications folder.

### Windows
Download the latest `.exe` setup file.

## Usage

1. **Target**: Select the directory you want to monitor (e.g., User Home).
2. **Drive**: Plug in a USB or External Drive. Select it from the list.
3. **Start**: Click **Start Monitoring**.
4. **Forget**: Tether runs in the background. Editing a file saves it to the drive instantly.

## Tech Stack
Built with **Rust** (Tauri) and **React** (TypeScript).

---
*v1.2.0 Enterprise*
