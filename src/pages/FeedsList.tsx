import { useState, useEffect, useCallback } from "react";
import type { Feed, SyncProgressEvent } from "../lib/types";
import { useCopyToClipboard } from "../hooks/useCopyToClipboard";
import { useTauriListener } from "../hooks/useTauriListener";
import ConfirmModal from "../components/ConfirmModal";
import * as api from "../lib/api";

type ViewMode = "list" | "collection";

interface Props {
  onSelectFeed: (feedId: string) => void;
  onAccount: () => void;
  syncStatus: string;
}

export default function FeedsList({ onSelectFeed, onAccount, syncStatus }: Props) {
  const [feeds, setFeeds] = useState<Feed[] | null>(null);
  const [search, setSearch] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>(() => {
    return (localStorage.getItem("feedsViewMode") as ViewMode) || "collection";
  });
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

  if (!feeds) {
    return (
      <div className="center">
        <img className="loading-spinner" src="/loading.svg" alt="Loading" />
      </div>
    );
  }

  const visibleFeeds = feeds.filter((f) => {
    if (!search) return true;
    const q = search.toLowerCase();
    return f.name.toLowerCase().includes(q) || f.source_url.toLowerCase().includes(q);
  });

  return (
    <div className="page feed-detail-page">
      <div className="feed-detail-header">
        <header className="toolbar">
          <h2>Feeds ({feeds.length})</h2>
          <div className="toolbar-actions">
            <div className="view-toggle">
              <button
                className={`btn small${viewMode === "list" ? " active" : ""}`}
                onClick={() => { setViewMode("list"); localStorage.setItem("feedsViewMode", "list"); }}
                title="List view"
              >
                ☰
              </button>
              <button
                className={`btn small${viewMode === "collection" ? " active" : ""}`}
                onClick={() => { setViewMode("collection"); localStorage.setItem("feedsViewMode", "collection"); }}
                title="Collection view"
              >
                ▦
              </button>
            </div>
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

      {viewMode === "list" ? (
        <ul className="feed-list episode-list-scroll">
          {visibleFeeds.map((feed) => (
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
          <li className="feed-item feed-item-add" onClick={() => setShowAdd(true)}>
            <div className="feed-info">
              <strong>+ Add Feed</strong>
            </div>
          </li>
        </ul>
      ) : (
        <div className="feed-collection episode-list-scroll">
          {visibleFeeds.map((feed) => (
            <div
              key={feed.id}
              className="feed-card"
              onClick={() => onSelectFeed(feed.id)}
            >
              <div className="feed-card-art">
                {feed.artwork_url ? (
                  <img src={feed.artwork_url} alt={feed.name} />
                ) : (
                  <div className="feed-card-placeholder">{feed.name.charAt(0)}</div>
                )}
              </div>
              <div className="feed-card-name">{feed.name}</div>
              <div className="feed-card-sub">{feed.episode_count ?? 0} ep{(feed.episode_count ?? 0) !== 1 ? "s" : ""}</div>
            </div>
          ))}
          <div
            className="feed-card feed-card-add"
            onClick={() => setShowAdd(true)}
          >
            <div className="feed-card-art">
              <div className="feed-card-placeholder">+</div>
            </div>
            <div className="feed-card-name">Add Feed</div>
          </div>
        </div>
      )}

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
  // const [fetchOrder, setFetchOrder] = useState("newest");
  const fetchOrder = "newest";
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
        description || undefined,
        fetchOrder
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
