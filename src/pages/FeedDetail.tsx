import { useState, useEffect } from "react";
import type { Episode, FeedDetailResponse, SyncProgressEvent, User } from "../lib/types";
import { useCopyToClipboard } from "../hooks/useCopyToClipboard";
import { useTauriListener } from "../hooks/useTauriListener";
import ConfirmModal from "../components/ConfirmModal";
import ListenOnButtons from "../components/ListenOnButtons";
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

function formatDate(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return "";
  return d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export default function FeedDetail({ feedId, user, onBack }: Props) {
  const limits = user.limits;
  const [detail, setDetail] = useState<FeedDetailResponse | null>(null);
  const [search, setSearch] = useState("");
  const [syncing, setSyncing] = useState(false);
  const [backfilling, setBackfilling] = useState(false);
  const [backfillCursor, setBackfillCursor] = useState(20);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [showConfirmDelete, setShowConfirmDelete] = useState(false);
  const { copiedId, copy } = useCopyToClipboard();

  const load = () => {
    api.getFeedDetail(feedId).then(setDetail).catch(console.error);
  };

  useEffect(() => {
    load();
  }, [feedId]);

  // Refresh when an episode in this feed completes
  useTauriListener<SyncProgressEvent>("sync-progress", (event) => {
    if (event.payload.feed_id === feedId && event.payload.step === "complete") {
      load();
    }
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

  const handleBackfill = async () => {
    const BATCH = 30;
    setBackfilling(true);
    try {
      await api.backfillFeed(feedId, backfillCursor, backfillCursor + BATCH);
      setBackfillCursor((prev) => prev + BATCH);
      load();
    } catch (err) {
      console.error(err);
    } finally {
      setBackfilling(false);
    }
  };

  const handleDelete = async () => {
    try {
      await api.deleteFeed(feedId);
      setShowConfirmDelete(false);
      onBack();
    } catch (err) {
      setShowConfirmDelete(false);
      setDeleteError(String(err));
    }
  };

  if (!detail) {
    return (
      <div className="center">
        <img className="loading-spinner" src="/loading.svg" alt="Loading" />
      </div>
    );
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
            <button className="btn danger" onClick={() => setShowConfirmDelete(true)}>
              Delete Feed
            </button>
          </div>
        </header>

        <div className="feed-meta">
          <span>{detail.episodes.length} episode{detail.episodes.length !== 1 ? "s" : ""}</span>
          <ListenOnButtons feedUrl={detail.feed_url} onCopyRss={() => copy(detail.feed_url)} rssCopied={!!copiedId} />
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

        <input
          className="search-input"
          placeholder="Search episodes..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <ul className="episode-list episode-list-scroll">
        {detail.episodes.filter((ep) => {
          if (!search) return true;
          return ep.title.toLowerCase().includes(search.toLowerCase());
        }).map((ep: Episode) => (
          <li key={ep.id} className="episode-item">
            <div className="episode-info">
              <strong>{ep.title}</strong>
              <span className="secondary">
                {ep.pub_date && formatDate(ep.pub_date)}
                {ep.pub_date && ep.duration_sec ? " \u00b7 " : ""}
                {ep.duration_sec ? formatDuration(ep.duration_sec) : ""}
              </span>
            </div>
            <div className="episode-status">
              <span
                className={`badge ${ep.status === "ready" ? "badge-ok" : "badge-warn"}`}
              >
                {ep.status === "ready" ? "ready" : "processing"}
              </span>
            </div>
          </li>
        ))}
        {detail.episodes.length === 0 && (
          <li className="empty">No episodes yet. Click Refresh to sync.</li>
        )}
      </ul>

      <div className="fetch-history-section">
        <button className="btn" onClick={handleBackfill} disabled={backfilling || syncing}>
          {backfilling ? "Fetching..." : "Fetch More Episodes"}
        </button>
      </div>

      {showConfirmDelete && (
        <ConfirmModal
          title={`Delete "${detail.feed.name}"?`}
          message="This will remove the feed and all its episodes."
          onConfirm={handleDelete}
          onCancel={() => setShowConfirmDelete(false)}
        />
      )}
    </div>
  );
}
