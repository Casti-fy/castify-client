import { useState, useEffect, useCallback } from "react";
import type { Feed } from "../lib/types";
import * as api from "../lib/api";

interface Props {
  onSelectFeed: (feedId: string) => void;
  onSettings: () => void;
  syncStatus: string;
}

const SERVER_URL = "http://es.alpharesearch.io:3000";

function feedUrl(slug: string) {
  return `${SERVER_URL}/rss/${slug}.xml`;
}

export default function FeedsList({ onSelectFeed, onSettings, syncStatus }: Props) {
  const [feeds, setFeeds] = useState<Feed[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

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

  const copyUrl = (slug: string) => {
    navigator.clipboard.writeText(feedUrl(slug));
  };

  return (
    <div className="page">
      <header className="toolbar">
        <h2>Feeds</h2>
        <div className="toolbar-actions">
          <button className="btn" onClick={() => setShowAdd(true)}>
            + Add Feed
          </button>
          <button className="btn" onClick={onSettings}>
            Settings
          </button>
        </div>
      </header>

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
              <span className="secondary">{feed.source_url}</span>
            </div>
            <div className="feed-actions">
              <button
                className="btn small"
                onClick={() => copyUrl(feed.feed_slug)}
              >
                Copy RSS
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
  const [createdSlug, setCreatedSlug] = useState<string | null>(null);

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
      setCreatedSlug(resp.feed.feed_slug);
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
            placeholder="YouTube URL (channel or playlist)"
            value={sourceUrl}
            onChange={(e) => setSourceUrl(e.target.value)}
          />
          <input
            placeholder="Description (optional)"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />

          {error && <p className="error">{error}</p>}

          {createdSlug && (
            <div className="success-box">
              <p>Feed created! RSS URL:</p>
              <div className="url-row">
                <code>{feedUrl(createdSlug)}</code>
                <button
                  type="button"
                  className="btn small"
                  onClick={() =>
                    navigator.clipboard.writeText(feedUrl(createdSlug))
                  }
                >
                  Copy
                </button>
              </div>
            </div>
          )}

          <div className="modal-actions">
            <button type="button" className="btn" onClick={onClose}>
              {createdSlug ? "Done" : "Cancel"}
            </button>
            {!createdSlug && (
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
