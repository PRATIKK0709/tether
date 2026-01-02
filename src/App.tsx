import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { homeDir } from "@tauri-apps/api/path";
import "./App.css";

interface DriveInfo {
  name: String;
  mount_point: String;
  total_space: number;
  available_space: number;
}

interface LogEntry {
  timestamp: string;
  message: string;
}

function App() {
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [watchPath, setWatchPath] = useState("");
  const [selectedDrive, setSelectedDrive] = useState<string | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isWatching, setIsWatching] = useState(false);

  const addLog = (msg: string) => {
    const time = new Date().toLocaleTimeString();
    setLogs((prev) => [{ timestamp: time, message: msg }, ...prev]);
  };

  const loadConfig = async () => {
    try {
      const c = await invoke<{ ignored_extensions: string[] }>("get_config");
      setConfig(c);
    } catch (e) {
      console.error(e);
      addLog(`Error loading config: ${e}`);
    }
  };

  const saveConfig = async (newConfig: { ignored_extensions: string[] }) => {
    try {
      await invoke("save_config", { config: newConfig });
      setConfig(newConfig);
      addLog("Settings updated.");
    } catch (e) {
      addLog(`Error saving settings: ${e}`);
    }
  };

  const addExtension = () => {
    if (!newExt) return;
    const clean = newExt.trim().toLowerCase().replace(/^\./, ""); // remove leading dot
    if (config.ignored_extensions.includes(clean)) return;

    const updated = { ...config, ignored_extensions: [...config.ignored_extensions, clean] };
    saveConfig(updated);
    setNewExt("");
  };

  const removeExtension = (ext: string) => {
    const updated = { ...config, ignored_extensions: config.ignored_extensions.filter(e => e !== ext) };
    saveConfig(updated);
  };

  useEffect(() => {
    // Initial fetch
    invoke<DriveInfo[]>("get_drives").then(setDrives);
    loadConfig();

    // Listeners
    const unlistenDrives = listen<DriveInfo[]>("drives-changed", (event) => {
      setDrives(event.payload);
      addLog("External drives updated.");
    });

    const unlistenSync = listen<string>("file-synced", (event) => {
      addLog(`Synced: ${event.payload}`);
    });

    const unlistenChange = listen<string[]>("file-changed", (event) => {
      // Optional: Log detected changes before sync
      addLog(`Detected changes: ${event.payload.length} files`);
    });

    return () => {
      unlistenDrives.then((f) => f());
      unlistenSync.then((f) => f());
      unlistenChange.then((f) => f());
    };
  }, []);

  const handleStartWatch = async () => {
    try {
      await invoke("start_watching", { path: watchPath });
      setIsWatching(true);
      addLog(`Started watching: ${watchPath}`);
    } catch (e) {
      addLog(`Error: ${e}`);
    }
  };

  const handleSelectDrive = async (path: any) => {
    setSelectedDrive(path);
    try {
      await invoke("set_backup_path", { path });
      addLog(`Backup target set to: ${path}`);
    } catch (e) {
      addLog(`Error setting backup: ${e}`);
    }
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected) {
        setWatchPath(selected as string);
      }
    } catch (e) {
      addLog(`Error picking directory: ${e}`);
    }
  };

  const handleTrackAll = async () => {
    try {
      const home = await homeDir();
      setWatchPath(home);
      addLog("Set target to User Home Directory");
    } catch (e) {
      addLog(`Error getting home dir: ${e}`);
    }
  };

  return (
    <div className="container">
      <header className="header">
        <div className="logo">Tether</div>
        <div className="version">v1.2.0</div>
      </header>

      <div className="card">
        <div className="card-header">Target Directory</div>
        <div className="input-group">
          <input
            type="text"
            placeholder="/Users/username/TopSecret"
            value={watchPath}
            onChange={(e) => setWatchPath(e.target.value)}
            disabled={isWatching}
          />
          <button className="icon-btn" onClick={handleBrowse} disabled={isWatching} title="Browse">
            📂
          </button>
        </div>
        <div className="actions-row">
          <button className="secondary-btn" onClick={handleTrackAll} disabled={isWatching}>
            Track User Home
          </button>
          <button
            className="primary-btn"
            onClick={handleStartWatch}
            disabled={isWatching || !watchPath || !selectedDrive}
            title={!selectedDrive ? "Select a drive first" : ""}
          >
            {isWatching ? "Active Query..." : "Start Monitoring"}
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card-header">Detected Drives</div>
        <div className="drive-list">
          {drives.length === 0 ? (
            <div style={{ color: "var(--text-secondary)", textAlign: "center", padding: "2rem", opacity: 0.5, fontSize: "0.8rem" }}>
              No Waiting Drives Found
            </div>
          ) : (
            drives.map((drive, idx) => (
              <div
                key={idx}
                className={`drive-item ${selectedDrive === drive.mount_point ? "active" : ""}`}
                onClick={() => handleSelectDrive(drive.mount_point)}
              >
                <div className="drive-info">
                  <span className="drive-name">{drive.name || "Untitled Drive"}</span>
                  <span className="drive-meta">{drive.mount_point}</span>
                </div>
                <div className="drive-stats">
                  {(drive.available_space / 1024 / 1024 / 1024).toFixed(1)} GB Free
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="card" style={{ flex: 1, display: "flex", flexDirection: "column" }}>
        <div className="card-header">Secure Audit Log</div>
        <div className="logs">
          {logs.map((log, idx) => (
            <div key={idx} className="log-entry">
              <span className="log-time">[{log.timestamp}]</span>
              <span className="log-message">{log.message}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default App;
