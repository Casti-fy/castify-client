export interface PlanLimits {
  max_feeds: number; // -1 = unlimited
  max_episodes_per_feed: number; // -1 = unlimited
  retention_days: number; // -1 = unlimited
  max_file_size: number;
  max_total_file_size: number;
}

export interface User {
  id: string;
  email: string;
  plan: string;
  limits: PlanLimits;
}

export interface Feed {
  id: string;
  name: string;
  source_url: string;
  description?: string;
  feed_slug: string;
  feed_url: string;
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
