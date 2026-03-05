import { useState } from "react";
import type { User } from "../lib/types";
import * as api from "../lib/api";

interface Props {
  user: User;
  onBack: () => void;
  onLogout: () => void;
}

const INTERVALS = [
  { label: "15 min", value: 15 },
  { label: "30 min", value: 30 },
  { label: "60 min", value: 60 },
  { label: "120 min", value: 120 },
];

export default function Settings({ user, onBack, onLogout }: Props) {
  const [interval, setInterval] = useState(30);

  const handleIntervalChange = (value: number) => {
    setInterval(value);
    api.stopPeriodicSync().then(() => api.startPeriodicSync(value)).catch(console.error);
  };

  const handleLogout = async () => {
    await api.logout();
    onLogout();
  };

  return (
    <div className="page">
      <header className="toolbar">
        <button className="btn" onClick={onBack}>
          &larr; Back
        </button>
        <h2>Settings</h2>
        <div className="toolbar-actions" />
      </header>

      <div className="settings-content">
        <div className="setting-row">
          <label>Auto-sync interval</label>
          <select
            value={interval}
            onChange={(e) => handleIntervalChange(Number(e.target.value))}
          >
            {INTERVALS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>

        <div className="setting-row">
          <span className="secondary">Logged in as {user.email}</span>
        </div>

        <button className="btn danger" onClick={handleLogout}>
          Logout
        </button>
      </div>
    </div>
  );
}
