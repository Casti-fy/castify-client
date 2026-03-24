import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-shell";
import { invoke } from "@tauri-apps/api/core";
import { isEnabled, enable, disable } from "@tauri-apps/plugin-autostart";
import type { User, PlanLimits } from "../lib/types";
import * as api from "../lib/api";
import { checkForAppUpdate, type UpdateProgress } from "../lib/updater";

interface Props {
  user: User;
  onBack: () => void;
  onLogout: () => void;
  onUserUpdate: (user: User) => void;
}

const PLAN_NAMES = ["starter", "pro", "unlimited"] as const;
const PLAN_LABELS: Record<string, string> = { starter: "Free", pro: "Pro", unlimited: "Unlimited" };

type BillingInterval = "month" | "year";

const PLAN_PRICING: Record<string, { month: number; year: number }> = {
  starter: { month: 0, year: 0 },
  pro: { month: 4.99, year: 49 },
  unlimited: { month: 8.99, year: 79 },
};

const SHOW_SUBSCRIPTION_SECTION = false;

function fmtPrice(plan: string, interval: BillingInterval): string {
  const p = PLAN_PRICING[plan];
  if (!p || p.month === 0) return "Free";
  if (interval === "month") return `$${p.month}/mo`;
  return `$${p.year}/yr`;
}

function annualDiscount(plan: string): number {
  const p = PLAN_PRICING[plan];
  if (!p || p.month === 0) return 0;
  const monthlyCost = p.month * 12;
  return Math.round((1 - p.year / monthlyCost) * 100);
}

const INTERVALS = [
  { label: "15 min", value: 15 },
  { label: "30 min", value: 30 },
  { label: "60 min", value: 60 },
  { label: "120 min", value: 120 },
];

function fmtLimit(v: number) { return v < 0 ? "\u221E" : String(v); }
function fmtRetention(d: number) { return d < 0 ? "Forever" : `${d}d`; }

export default function Account({ user, onBack, onLogout, onUserUpdate }: Props) {
  const [syncInterval, setSyncInterval] = useState(30);
  const [billingInterval, setBillingInterval] = useState<BillingInterval>("year");
  const [loading, setLoading] = useState<string | null>(null);
  const [launchAtStartup, setLaunchAtStartup] = useState(false);
  const [allPlanLimits, setAllPlanLimits] = useState<Record<string, PlanLimits> | null>(null);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgress | null>(null);

  // Load persisted sync interval, autostart setting, and plan limits
  useEffect(() => {
    isEnabled().then(setLaunchAtStartup).catch(() => {});
    api.getSyncInterval().then(setSyncInterval).catch(() => {});
    api.fetchPlans().then(setAllPlanLimits).catch(() => {});
  }, []);

  const handleSyncIntervalChange = (value: number) => {
    setSyncInterval(value);
    api.setSyncInterval(value).catch(console.error);
  };

  const handleUpgrade = async (plan: string) => {
    try {
      setLoading(plan);
      const url = await api.createCheckout(plan, billingInterval);
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

  const handleLaunchAtStartupChange = async (checked: boolean) => {
    try {
      if (checked) {
        await enable();
      } else {
        await disable();
      }
      setLaunchAtStartup(checked);
    } catch (e) {
      console.error("Launch at startup error:", e);
    }
  };

  const handleClearCache = async () => {
    try {
      await invoke("clear_sync_cache");
    } catch (e) {
      console.error("Clear cache error:", e);
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

        {SHOW_SUBSCRIPTION_SECTION && (
          <>
            <div className="billing-toggle">
              <button
                className={`btn small ${billingInterval === "month" ? "primary" : ""}`}
                onClick={() => setBillingInterval("month")}
              >
                Monthly
              </button>
              <button
                className={`btn small ${billingInterval === "year" ? "primary" : ""}`}
                onClick={() => setBillingInterval("year")}
              >
                Annual {annualDiscount("pro") > 0 && <span className="badge badge-discount">Save {annualDiscount("pro")}%</span>}
              </button>
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
                {allPlanLimits && ([
                  ["Price", (p: string) => fmtPrice(p, billingInterval)],
                  ["Feeds", (p: string) => fmtLimit(allPlanLimits[p]?.max_feeds ?? 0)],
                  ["Eps / feed", (p: string) => fmtLimit(allPlanLimits[p]?.max_episodes_per_feed ?? 0)],
                  ["Retention", (p: string) => fmtRetention(allPlanLimits[p]?.retention_days ?? 0)],
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
          </>
        )}

        <div className="setting-row">
          <label>Launch at startup</label>
          <label className="setting-toggle">
            <input
              type="checkbox"
              checked={launchAtStartup}
              onChange={(e) => handleLaunchAtStartupChange(e.target.checked)}
            />
            <span>{launchAtStartup ? "On" : "Off"}</span>
          </label>
        </div>

        <div className="setting-row">
          <label>Auto-sync</label>
          <select
            value={syncInterval}
            onChange={(e) => handleSyncIntervalChange(Number(e.target.value))}
          >
            {INTERVALS.map((opt) => {
              return (
                <option
                  key={opt.value}
                  value={opt.value}
                >
                  {opt.label}
                </option>
              );
            })}
          </select>
        </div>

        <div className="setting-row">
          <label>Downloaded audio cache</label>
          <button className="btn small" onClick={handleClearCache}>
            Clear cache
          </button>
        </div>

        <div className="setting-row">
          <label>Version</label>
          <span>{__APP_VERSION__}</span>
        </div>

        <div className="setting-row">
          <label>App updates</label>
          {updateProgress?.phase === "downloading" ? (
            <div className="update-progress">
              <div className="update-progress-bar">
                <div
                  className="update-progress-fill"
                  style={{ width: `${updateProgress.percent ?? 0}%` }}
                />
              </div>
              <span className="update-progress-label">
                {updateProgress.percent != null ? `${updateProgress.percent}%` : "Downloading..."}
              </span>
            </div>
          ) : (
            <button
              className="btn small"
              disabled={updateProgress !== null}
              onClick={() => {
                checkForAppUpdate(false, (p) => setUpdateProgress(p ?? null));
              }}
            >
              {updateProgress?.phase === "checking" ? "Checking..." :
               updateProgress?.phase === "installing" ? "Installing..." :
               "Check for updates"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
