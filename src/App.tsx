import { useState, useEffect } from "react";
import type { User, SyncProgressEvent } from "./lib/types";
import { checkAuth } from "./lib/api";
import { checkForAppUpdate, scheduleMidnightUTCCheck } from "./lib/updater";
import { useTauriListener } from "./hooks/useTauriListener";
import Login from "./pages/Login";
import FeedsList from "./pages/FeedsList";
import FeedDetail from "./pages/FeedDetail";
import Account from "./pages/Account";

type Page =
  | { name: "feeds" }
  | { name: "feed-detail"; feedId: string }
  | { name: "account" };

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

    checkForAppUpdate();
    const cleanup = scheduleMidnightUTCCheck();
    return cleanup;
  }, []);


  // Refresh user data when window regains focus (e.g. after Stripe checkout)
  useTauriListener("tauri://focus", () => {
    if (!user) return;
    checkAuth()
      .then((u) => setUser(u))
      .catch(() => {});
  }, [user]);

  // Listen for sync progress events
  useTauriListener<SyncProgressEvent>("sync-progress", (event) => {
    const { feed_name, step, message } = event.payload;
    if (step === "done") {
      setSyncStatus("");
    } else {
      setSyncStatus(`[${feed_name}] ${message}`);
    }
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
          onAccount={() => setPage({ name: "account" })}
          syncStatus={syncStatus}
        />
      )}
      {page.name === "feed-detail" && (
        <FeedDetail
          feedId={page.feedId}
          user={user}
          onBack={() => setPage({ name: "feeds" })}
        />
      )}
      {page.name === "account" && (
        <Account
          user={user}
          onBack={() => setPage({ name: "feeds" })}
          onLogout={() => {
            setUser(null);
            setPage({ name: "feeds" });
            setSyncStatus("");
          }}
          onUserUpdate={(u) => setUser(u)}
        />
      )}
    </div>
  );
}
