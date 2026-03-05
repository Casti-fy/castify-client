import { useState, useEffect } from "react";
import type { Episode, FeedDetailResponse } from "../lib/types";
import * as api from "../lib/api";

interface Props {
  feedId: string;
  onBack: () => void;
}

function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export default function FeedDetail({ feedId, onBack }: Props) {
  const [detail, setDetail] = useState<FeedDetailResponse | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

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
    <div className="page">
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

      <div className="feed-url-bar">
        <code>{detail.feed_url}</code>
        <button
          className="btn small"
          onClick={() => navigator.clipboard.writeText(detail.feed_url)}
        >
          Copy RSS
        </button>
      </div>

      {deleteError && <div className="error-banner">{deleteError}</div>}

      <ul className="episode-list">
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
