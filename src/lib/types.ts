export interface User {
  id: string;
  email: string;
  plan: string;
}

export interface PlanLimits {
  max_feeds: number; // -1 = unlimited
  max_episodes_per_feed: number; // -1 = unlimited
  retention_days: number; // -1 = unlimited
}

export const PLAN_LIMITS: Record<string, PlanLimits> = {
  starter: { max_feeds: 1, max_episodes_per_feed: 10, retention_days: 30 },
  pro: { max_feeds: 20, max_episodes_per_feed: -1, retention_days: -1 },
  unlimited: { max_feeds: -1, max_episodes_per_feed: -1, retention_days: -1 },
};

export function getPlanLimits(plan: string): PlanLimits {
  return PLAN_LIMITS[plan] || PLAN_LIMITS.starter;
}

export interface Feed {
  id: string;
  name: string;
  source_url: string;
  description?: string;
  feed_slug: string;
  episode_count?: number;
}

export interface Episode {
  id: string;
  feed_id: string;
  video_id: string;
  title: string;
  description?: string;
  pub_date?: string;
  duration_sec?: number;
  status: string;
}

export interface AuthResponse {
  token: string;
  user: User;
}

export interface CreateFeedResponse {
  feed: Feed;
  feed_url: string;
}

export interface FeedDetailResponse {
  feed: Feed;
  episodes: Episode[];
  feed_url: string;
}

export interface SyncProgressEvent {
  feed_id: string;
  feed_name: string;
  step: string;
  message: string;
}
