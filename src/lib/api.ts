import { invoke } from "@tauri-apps/api/core";
import type {
  User,
  Feed,
  AuthResponse,
  CreateFeedResponse,
  FeedDetailResponse,
} from "./types";

// Auth
export const login = (email: string, password: string) =>
  invoke<AuthResponse>("login", { email, password });

export const register = (email: string, password: string) =>
  invoke<AuthResponse>("register", { email, password });

export const checkAuth = () => invoke<User>("check_auth");

export const logout = () => invoke<void>("logout");

// Feeds
export const listFeeds = () => invoke<Feed[]>("list_feeds");

export const createFeed = (
  name: string,
  sourceUrl: string,
  description?: string
) =>
  invoke<CreateFeedResponse>("create_feed", {
    name,
    sourceUrl,
    description: description || null,
  });

export const getFeedDetail = (feedId: string) =>
  invoke<FeedDetailResponse>("get_feed_detail", { feedId });

export const deleteFeed = (feedId: string) =>
  invoke<void>("delete_feed", { feedId });

// Sync
export const syncFeed = (feedId: string) =>
  invoke<void>("sync_feed", { feedId });

export const startPeriodicSync = (intervalMinutes: number) =>
  invoke<void>("start_periodic_sync", { intervalMinutes });

export const stopPeriodicSync = () => invoke<void>("stop_periodic_sync");

// Billing
export const createCheckout = (plan: string, interval: "month" | "year") =>
  invoke<string>("create_checkout", { plan, interval });

export const createPortal = () => invoke<string>("create_portal");
