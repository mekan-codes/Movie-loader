import { Download, RotateCcw, Save, Upload } from "lucide-react";
import { useState } from "react";
import type { AppExport, AppSettings } from "../types";
import { downloadJson, readJsonFile } from "../utils";

interface SettingsViewProps {
  settings: AppSettings;
  onSettingsChange: (settings: AppSettings) => void;
  onClearCache: () => void;
  onExportData: () => AppExport;
  onImportData: (data: AppExport) => Promise<void>;
}

export function SettingsView({
  settings,
  onSettingsChange,
  onClearCache,
  onExportData,
  onImportData
}: SettingsViewProps) {
  const [message, setMessage] = useState<string | null>(null);

  const update = <Key extends keyof AppSettings>(key: Key, value: AppSettings[Key]) => {
    onSettingsChange({ ...settings, [key]: value });
    setMessage("Settings saved.");
  };

  const importFile = async (file: File) => {
    const data = await readJsonFile<AppExport>(file);
    await onImportData(data);
    setMessage("App data imported.");
  };

  return (
    <section className="view settings-view">
      <header className="view-header">
        <div>
          <h1>Settings</h1>
          <p>Local preferences</p>
        </div>
      </header>

      <div className="settings-grid">
        <section className="settings-panel">
          <h2>Defaults</h2>
          <label>
            <span>Search behavior</span>
            <select
              value={settings.defaultSearchBehavior}
              onChange={(event) =>
                update("defaultSearchBehavior", event.target.value as AppSettings["defaultSearchBehavior"])
              }
            >
              <option value="enabled">Enabled sources</option>
              <option value="lastSelected">Last selected sources</option>
            </select>
          </label>
          <label>
            <span>Player mode</span>
            <select
              value={settings.defaultPlayerMode}
              onChange={(event) =>
                update("defaultPlayerMode", event.target.value as AppSettings["defaultPlayerMode"])
              }
            >
              <option value="webview">WebView</option>
              <option value="native">Native</option>
              <option value="ask">Ask</option>
            </select>
          </label>
          <label>
            <span>Open behavior</span>
            <select
              value={settings.openBehavior}
              onChange={(event) =>
                update("openBehavior", event.target.value as AppSettings["openBehavior"])
              }
            >
              <option value="inApp">In-app viewer</option>
              <option value="external">External browser</option>
            </select>
          </label>
          <label>
            <span>Theme</span>
            <select
              value={settings.theme}
              onChange={(event) => update("theme", event.target.value as AppSettings["theme"])}
            >
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
          <label>
            <span>Download folder</span>
            <input
              value={settings.defaultDownloadFolder}
              onChange={(event) => update("defaultDownloadFolder", event.target.value)}
              placeholder="C:\\Users\\you\\Downloads"
            />
          </label>
          <label>
            <span>Web open delay</span>
            <input
              type="number"
              min={0}
              max={15}
              step={1}
              value={settings.webOpenDelaySeconds}
              onChange={(event) =>
                update(
                  "webOpenDelaySeconds",
                  clampDelaySeconds(Number(event.target.value))
                )
              }
            />
          </label>
          <button className="primary-button" type="button">
            <Save size={17} />
            <span>Saved</span>
          </button>
        </section>

        <section className="settings-panel">
          <h2>Data</h2>
          <div className="settings-actions">
            <button
              className="secondary-button"
              type="button"
              onClick={() =>
                downloadJson(
                  `cinefinder-data-${new Date().toISOString().slice(0, 10)}.json`,
                  onExportData()
                )
              }
            >
              <Download size={17} />
              <span>Export</span>
            </button>
            <label className="secondary-button file-button">
              <Upload size={17} />
              <span>Import</span>
              <input
                type="file"
                accept="application/json,.json"
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file) {
                    void importFile(file);
                  }
                  event.currentTarget.value = "";
                }}
              />
            </label>
            <button
              className="secondary-button"
              type="button"
              onClick={() => {
                onClearCache();
                setMessage("Recent-search cache cleared.");
              }}
            >
              <RotateCcw size={17} />
              <span>Clear cache</span>
            </button>
          </div>
          {message && <div className="form-message">{message}</div>}
        </section>
      </div>
    </section>
  );
}

function clampDelaySeconds(value: number): number {
  if (!Number.isFinite(value)) {
    return 5;
  }
  return Math.min(15, Math.max(0, Math.round(value)));
}
