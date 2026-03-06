import { useState } from "react";
import { open } from "@tauri-apps/plugin-shell";
import type { User } from "../lib/types";
import { PLAN_LIMITS } from "../lib/types";
import * as api from "../lib/api";

interface Props {
  user: User;
  onBack: () => void;
  onLogout: () => void;
  onUserUpdate: (user: User) => void;
}

const PLAN_NAMES = ["starter", "pro", "unlimited"] as const;
const PLAN_LABELS: Record<string, string> = { starter: "Starter", pro: "Pro", unlimited: "Unlimited" };
const PLAN_PRICES: Record<string, string> = { starter: "Free", pro: "$6.99/mo", unlimited: "$9.99/mo" };

const INTERVALS = [
  { label: "15 min", value: 15 },
  { label: "30 min", value: 30 },
  { label: "60 min", value: 60 },
  { label: "120 min", value: 120 },
];

function fmtLimit(v: number) { return v < 0 ? "\u221E" : String(v); }
function fmtRetention(d: number) { return d < 0 ? "Forever" : `${d}d`; }

export default function Account({ user, onBack, onLogout, onUserUpdate }: Props) {
  const [interval, setInterval] = useState(30);
  const [loading, setLoading] = useState<string | null>(null);

  const handleIntervalChange = (value: number) => {
    setInterval(value);
    api.stopPeriodicSync().then(() => api.startPeriodicSync(value)).catch(console.error);
  };

  const handleUpgrade = async (plan: string) => {
    try {
      setLoading(plan);
      const url = await api.createCheckout(plan);
      console.log("Checkout URL:", url);
      if (url) {
        await open(url);
      }
    } catch (e: any) {
      console.error("Checkout error:", e);
    } finally {
      setLoading(null);
    }
  };

  const handleManageSubscription = async () => {
    try {
      setLoading("portal");
      const url = await api.createPortal();
      await open(url);
    } catch (e: any) {
      console.error("Portal error:", e);
    } finally {
      setLoading(null);
    }
  };

  const handleRefreshPlan = async () => {
    try {
      const updatedUser = await api.checkAuth();
      onUserUpdate(updatedUser);
    } catch (e) {
      console.error("Refresh error:", e);
    }
  };

  return (
    <div className="page">
      <header className="toolbar">
        <button className="btn" onClick={onBack}>&larr; Back</button>
        <h2>Account</h2>
        <div className="toolbar-actions">
          <button className="btn danger" onClick={() => { api.logout().then(onLogout); }}>
            Sign out
          </button>
        </div>
      </header>

      <div className="account-content">
        <div className="account-header">
          <span>{user.email}</span>
          <span className="plan-badge-current">{PLAN_LABELS[user.plan] || user.plan}</span>
        </div>

        <table className="plan-table">
          <thead>
            <tr>
              <th />
              {PLAN_NAMES.map((p) => (
                <th key={p} className={p === user.plan ? "plan-active" : ""}>{PLAN_LABELS[p]}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {([
              ["Price", (p: string) => PLAN_PRICES[p]],
              ["Feeds", (p: string) => fmtLimit(PLAN_LIMITS[p].max_feeds)],
              ["Eps / feed", (p: string) => fmtLimit(PLAN_LIMITS[p].max_episodes_per_feed)],
              ["Retention", (p: string) => fmtRetention(PLAN_LIMITS[p].retention_days)],
            ] as [string, (p: string) => string][]).map(([label, fn]) => (
              <tr key={label}>
                <td>{label}</td>
                {PLAN_NAMES.map((p) => (
                  <td key={p} className={p === user.plan ? "plan-active" : ""}>{fn(p)}</td>
                ))}
              </tr>
            ))}
            <tr>
              <td />
              {PLAN_NAMES.map((p) => (
                <td key={p} className={p === user.plan ? "plan-active" : ""}>
                  {p === user.plan ? (
                    <span className="badge badge-ok">Current</span>
                  ) : p === "starter" ? (
                    user.plan !== "starter" ? (
                      <button className="btn small" onClick={handleManageSubscription} disabled={loading !== null}>
                        {loading === "portal" ? "..." : "Manage"}
                      </button>
                    ) : null
                  ) : (
                    PLAN_NAMES.indexOf(p) > PLAN_NAMES.indexOf(user.plan as typeof PLAN_NAMES[number]) ? (
                      <button
                        className="btn small primary"
                        onClick={() => handleUpgrade(p)}
                        disabled={loading !== null}
                      >
                        {loading === p ? "..." : "Upgrade"}
                      </button>
                    ) : (
                      <button className="btn small" onClick={handleManageSubscription} disabled={loading !== null}>
                        {loading === "portal" ? "..." : "Manage"}
                      </button>
                    )
                  )}
                </td>
              ))}
            </tr>
          </tbody>
        </table>

        {user.plan !== "starter" && (
          <div className="setting-row">
            <button className="btn small" onClick={handleManageSubscription} disabled={loading !== null}>
              {loading === "portal" ? "..." : "Manage subscription"}
            </button>
            <button className="btn small" onClick={handleRefreshPlan}>
              Refresh plan
            </button>
          </div>
        )}

        <div className="setting-row">
          <label>Auto-sync</label>
          <select value={interval} onChange={(e) => handleIntervalChange(Number(e.target.value))}>
            {INTERVALS.map((opt) => (
              <option key={opt.value} value={opt.value}>{opt.label}</option>
            ))}
          </select>
        </div>
      </div>
    </div>
  );
}
