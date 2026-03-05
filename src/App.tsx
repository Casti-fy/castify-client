import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { User, SyncProgressEvent } from "./lib/types";
import { checkAuth, startPeriodicSync } from "./lib/api";
import Login from "./pages/Login";
import FeedsList from "./pages/FeedsList";
import FeedDetail from "./pages/FeedDetail";
import Settings from "./pages/Settings";

type Page =
  | { name: "feeds" }
  | { name: "feed-detail"; feedId: string }
  | { name: "settings" };

export default function App() {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [page, setPage] = useState<Page>({ name: "feeds" });
  const [syncStatus, setSyncStatus] = useState<string>("");

  useEffect(() => {
    checkAuth()
      .then((u) => setUser(u))
      .catch(() => setUser(null))
      .finally(() => setLoading(false));
  }, []);

  // Start periodic sync when logged in
  useEffect(() => {
    if (user) {
      startPeriodicSync(30).catch(console.error);
    }
  }, [user]);

  // Listen for sync progress events
  useEffect(() => {
    const unlisten = listen<SyncProgressEvent>("sync-progress", (event) => {
      const { feed_name, step, message } = event.payload;
      if (step === "done") {
        setSyncStatus("");
      } else {
        setSyncStatus(`[${feed_name}] ${message}`);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (loading) {
    return <div className="center">Loading...</div>;
  }

  if (!user) {
    return (
      <Login
        onLogin={(u) => {
          setUser(u);
          setPage({ name: "feeds" });
        }}
      />
    );
  }

  return (
    <div className="app">
      {page.name === "feeds" && (
        <FeedsList
          onSelectFeed={(id) => setPage({ name: "feed-detail", feedId: id })}
          onSettings={() => setPage({ name: "settings" })}
          syncStatus={syncStatus}
        />
      )}
      {page.name === "feed-detail" && (
        <FeedDetail
          feedId={page.feedId}
          onBack={() => setPage({ name: "feeds" })}
        />
      )}
      {page.name === "settings" && (
        <Settings
          user={user}
          onBack={() => setPage({ name: "feeds" })}
          onLogout={() => {
            setUser(null);
            setPage({ name: "feeds" });
          }}
        />
      )}
    </div>
  );
}
