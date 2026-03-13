import { useState, useEffect, useCallback } from "react";
import type { Feed, User } from "../lib/types";
import { useCopyToClipboard } from "../hooks/useCopyToClipboard";
import * as api from "../lib/api";

interface Props {
  user: User;
  onSelectFeed: (feedId: string) => void;
  onAccount: () => void;
  syncStatus: string;
}

export default function FeedsList({ user, onSelectFeed, onAccount, syncStatus }: Props) {
  const [feeds, setFeeds] = useState<Feed[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const { copiedId, copy } = useCopyToClipboard();

  const limits = user.limits;
  const atFeedLimit = limits.max_feeds >= 0 && feeds.length >= limits.max_feeds;

  const load = useCallback(() => {
    api.listFeeds().then(setFeeds).catch(console.error);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const handleDelete = async (feed: Feed) => {
    if (!confirm(`Delete "${feed.name}"? This will remove the feed and all its episodes.`)) return;
    try {
      await api.deleteFeed(feed.id);
      load();
    } catch (err) {
      setDeleteError(String(err));
    }
  };

  return (
    <div className="page">
      <header className="toolbar">
        <h2>Feeds ({feeds.length}{limits.max_feeds >= 0 ? `/${limits.max_feeds}` : ""})</h2>
        <div className="toolbar-actions">
          <button
            className="btn"
            onClick={() => setShowAdd(true)}
            disabled={atFeedLimit}
            title={atFeedLimit ? `Feed limit reached (${limits.max_feeds}). Upgrade your plan.` : undefined}
          >
            + Add Feed
          </button>
          <button className="btn" onClick={onAccount}>
            Account
          </button>
        </div>
      </header>

      {atFeedLimit && (
        <div className="plan-banner">
          You've reached the {user.plan} plan limit of {limits.max_feeds} feed{limits.max_feeds > 1 ? "s" : ""}.
          Upgrade for more.
        </div>
      )}

      {syncStatus && <div className="sync-status">{syncStatus}</div>}

      {deleteError && (
        <div className="error-banner">
          {deleteError}
          <button className="btn link" onClick={() => setDeleteError(null)}>
            Dismiss
          </button>
        </div>
      )}

      <ul className="feed-list">
        {feeds.map((feed) => (
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
                onClick={() => handleDelete(feed)}
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
