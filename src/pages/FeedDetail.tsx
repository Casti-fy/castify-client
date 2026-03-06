import { useState, useEffect } from "react";
import type { Episode, FeedDetailResponse, User } from "../lib/types";
import { getPlanLimits } from "../lib/types";
import * as api from "../lib/api";

interface Props {
  feedId: string;
  user: User;
  onBack: () => void;
}

function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export default function FeedDetail({ feedId, user, onBack }: Props) {
  const limits = getPlanLimits(user.plan);
  const [detail, setDetail] = useState<FeedDetailResponse | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const load = () => {
    api.getFeedDetail(feedId).then(setDetail).catch(console.error);
  };

  useEffect(() => {
    load();
  }, [feedId]);

  const handleSync = async () => {
    setSyncing(true);
    try {
      await api.syncFeed(feedId);
      load();
    } catch (err) {
      console.error(err);
    } finally {
      setSyncing(false);
    }
  };

  const handleDelete = async () => {
    if (
      !confirm(
        `Delete "${detail?.feed.name}"? This will remove the feed and all its episodes.`
      )
    )
      return;
    try {
      await api.deleteFeed(feedId);
      onBack();
    } catch (err) {
      setDeleteError(String(err));
    }
  };

  if (!detail) {
    return <div className="center">Loading...</div>;
  }

  return (
    <div className="page feed-detail-page">
      <div className="feed-detail-header">
        <header className="toolbar">
          <button className="btn" onClick={onBack}>
            &larr; Back
          </button>
          <h2>{detail.feed.name}</h2>
          <div className="toolbar-actions">
            <button className="btn" onClick={handleSync} disabled={syncing}>
              {syncing ? "Syncing..." : "Refresh"}
            </button>
            <button className="btn danger" onClick={handleDelete}>
              Delete Feed
            </button>
          </div>
        </header>

        <div className="feed-meta">
          <span>{detail.episodes.length} episode{detail.episodes.length !== 1 ? "s" : ""}</span>
        </div>

        <div className={`feed-url-bar${copied ? " feed-url-copied" : ""}`}>
          <code>{detail.feed_url}</code>
          <button
            className="btn small"
            onClick={() => {
              navigator.clipboard.writeText(detail.feed_url);
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
          >
            {copied ? "Copied!" : "Copy RSS"}
          </button>
        </div>

        {limits.retention_days > 0 && (
          <div className="plan-banner">
            Episodes expire after {limits.retention_days} days on the {user.plan} plan.
            Upgrade to keep them forever.
          </div>
        )}

        {limits.max_episodes_per_feed > 0 && detail.episodes.length >= limits.max_episodes_per_feed && (
          <div className="plan-banner">
            Episode limit reached ({limits.max_episodes_per_feed}).
            Upgrade to sync the full backlog.
          </div>
        )}

        {deleteError && <div className="error-banner">{deleteError}</div>}
      </div>

      <ul className="episode-list episode-list-scroll">
        {detail.episodes.map((ep: Episode) => (
          <li key={ep.id} className="episode-item">
            <div className="episode-info">
              <strong>{ep.title}</strong>
              {ep.duration_sec && (
                <span className="secondary">
                  {formatDuration(ep.duration_sec)}
                </span>
              )}
            </div>
            <div className="episode-status">
              <span
                className={`badge ${ep.status === "ready" ? "badge-ok" : "badge-warn"}`}
              >
                {ep.status}
              </span>
            </div>
          </li>
        ))}
        {detail.episodes.length === 0 && (
          <li className="empty">No episodes yet. Click Refresh to sync.</li>
        )}
      </ul>
    </div>
  );
}
