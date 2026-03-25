import { useState, useEffect, useCallback } from "react";
import type { Feed, SyncProgressEvent } from "../lib/types";
import { useCopyToClipboard } from "../hooks/useCopyToClipboard";
import { useTauriListener } from "../hooks/useTauriListener";
import ConfirmModal from "../components/ConfirmModal";
import * as api from "../lib/api";

interface Props {
  onSelectFeed: (feedId: string) => void;
  onAccount: () => void;
  syncStatus: string;
}

export default function FeedsList({ onSelectFeed, onAccount, syncStatus }: Props) {
  const [feeds, setFeeds] = useState<Feed[]>([]);
  const [search, setSearch] = useState("");
  const [showAdd, setShowAdd] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<Feed | null>(null);
  const { copiedId, copy } = useCopyToClipboard();

  const load = useCallback(() => {
    api.listFeeds().then(setFeeds).catch(console.error);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  // Refresh feed list when any episode completes (episode_count may change)
  useTauriListener<SyncProgressEvent>("sync-progress", (event) => {
    if (event.payload.step === "complete") {
      load();
    }
  }, [load]);

  const handleDelete = async (feed: Feed) => {
    try {
      await api.deleteFeed(feed.id);
      setConfirmDelete(null);
      load();
    } catch (err) {
      setConfirmDelete(null);
      setDeleteError(String(err));
    }
  };

  return (
    <div className="page feed-detail-page">
      <div className="feed-detail-header">
        <header className="toolbar">
          <h2>Feeds ({feeds.length})</h2>
          <div className="toolbar-actions">
            <button
              className="btn"
              onClick={() => setShowAdd(true)}
            >
              + Add Feed
            </button>
            <button className="btn" onClick={onAccount}>
              Account
            </button>
          </div>
        </header>

        <input
          className="search-input"
          placeholder="Search feeds..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />

        {syncStatus && <div className="sync-status">{syncStatus}</div>}

        {deleteError && (
          <div className="error-banner">
            {deleteError}
            <button className="btn link" onClick={() => setDeleteError(null)}>
              Dismiss
            </button>
          </div>
        )}
      </div>

      <ul className="feed-list episode-list-scroll">
        {feeds.filter((f) => {
          if (!search) return true;
          const q = search.toLowerCase();
          return f.name.toLowerCase().includes(q) || f.source_url.toLowerCase().includes(q);
        }).map((feed) => (
          <li key={feed.id} className="feed-item">
            <div
              className="feed-info"
              onClick={() => onSelectFeed(feed.id)}
            >
              <strong>{feed.name}</strong>
              <span className="secondary">{feed.episode_count ?? 0} episode{feed.episode_count !== 1 ? "s" : ""} &middot; {feed.source_url}</span>
            </div>
            <div className="feed-actions">
              <button
                className={`btn small${copiedId === feed.id ? " btn-copied" : ""}`}
                onClick={() => copy(feed.feed_url, feed.id)}
              >
                {copiedId === feed.id ? "Copied!" : "Copy RSS"}
              </button>
              <button
                className="btn small danger"
                onClick={() => setConfirmDelete(feed)}
              >
                Delete
              </button>
            </div>
          </li>
        ))}
        {feeds.length === 0 && (
          <li className="empty">No feeds yet. Add one to get started.</li>
        )}
      </ul>

      {showAdd && (
        <AddFeedModal
          onClose={() => {
            setShowAdd(false);
            load();
          }}
        />
      )}

      {confirmDelete && (
        <ConfirmModal
          title={`Delete "${confirmDelete.name}"?`}
          message="This will remove the feed and all its episodes."
          onConfirm={() => handleDelete(confirmDelete)}
          onCancel={() => setConfirmDelete(null)}
        />
      )}
    </div>
  );
}

function AddFeedModal({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("");
  const [sourceUrl, setSourceUrl] = useState("");
  const [description, setDescription] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createdUrl, setCreatedUrl] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      const resp = await api.createFeed(
        name,
        sourceUrl,
        description || undefined
      );
      setCreatedUrl(resp.feed_url);
      // Trigger sync in background
      api.syncFeed(resp.feed.id).catch(console.error);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>Add Feed</h3>
        <form onSubmit={submit}>
          <input
            placeholder="Name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
          />
          <input
            placeholder="Source URL (YouTube, SoundCloud, ...)"
            value={sourceUrl}
            onChange={(e) => setSourceUrl(e.target.value)}
          />
          <input
            placeholder="Description (optional)"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />

          {error && <p className="error">{error}</p>}

          {createdUrl && (
            <div className="success-box">
              <p>Feed created! RSS URL:</p>
              <div className="url-row">
                <code>{createdUrl}</code>
                <button
                  type="button"
                  className="btn small"
                  onClick={() =>
                    navigator.clipboard.writeText(createdUrl)
                  }
                >
                  Copy
                </button>
              </div>
            </div>
          )}

          <div className="modal-actions">
            <button type="button" className="btn" onClick={onClose}>
              {createdUrl ? "Done" : "Cancel"}
            </button>
            {!createdUrl && (
              <button
                type="submit"
                className="btn primary"
                disabled={loading || !name || !sourceUrl}
              >
                {loading ? "Creating..." : "Create"}
              </button>
            )}
          </div>
        </form>
      </div>
    </div>
  );
}
