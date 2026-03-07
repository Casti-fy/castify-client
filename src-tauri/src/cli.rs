//! CLI entry: credential store (file-based), server URL from config/env/default,
//! and async API calls via ApiClient.

use clap::{Parser, Subcommand};

use crate::error::AppError;
use crate::models::{AuthResponse, CreateFeedRequest, CreateFeedResponse, Feed, LoginRequest, User};
use crate::services::api_client::ApiClient;
use crate::storage::{CredentialStore, FileStore};
use crate::DEFAULT_SERVER_URL;

#[derive(Parser)]
#[command(name = "castify")]
#[command(about = "Castify – sync podcast feeds", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Server URL (overrides config and env CASTIFY_SERVER_URL)
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Output JSON where applicable
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Log in (store token for later use)
    Login {
        #[arg(short, long)]
        email: Option<String>,
        /// Password (omit to prompt securely)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Log out (clear stored token)
    Logout,
    /// Show auth and server status
    Status,
    /// Manage feeds
    Feed {
        #[command(subcommand)]
        sub: FeedSub,
    },
    /// Run sync once (desktop app only for now)
    Sync,
}

#[derive(Subcommand)]
pub enum FeedSub {
    /// List feeds
    List,
    /// Add a feed
    Add {
        #[arg(short, long)]
        title: String,
        #[arg(short, long)]
        url: String,
        #[arg(long)]
        description: Option<String>,
    },
}

/// Resolve server URL: CLI arg > env CASTIFY_SERVER_URL > store > default.
fn server_url(cli: &Cli, store: &impl CredentialStore) -> Result<String, AppError> {
    if let Some(ref u) = cli.server {
        return Ok(u.clone());
    }
    if let Ok(u) = std::env::var("CASTIFY_SERVER_URL") {
        if !u.is_empty() {
            return Ok(u);
        }
    }
    if let Some(u) = store.get_server_url()? {
        if !u.is_empty() {
            return Ok(u);
        }
    }
    Ok(DEFAULT_SERVER_URL.to_string())
}

fn run_async<F, T>(f: F) -> Result<T, AppError>
where
    F: std::future::Future<Output = Result<T, AppError>>,
{
    let rt = tokio::runtime::Runtime::new().map_err(|e| AppError::Other(e.to_string()))?;
    rt.block_on(f)
}

/// Entry point for CLI. Returns exit code.
pub fn run(args: impl IntoIterator<Item = String>) -> i32 {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return e.exit_code().into();
        }
    };

    let store = match FileStore::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Config error: {}", e);
            return 1;
        }
    };

    let base_url = match server_url(&cli, &store) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("Server URL error: {}", e);
            return 1;
        }
    };

    match cli.command {
        None => {
            let _ = Cli::parse_from(["castify", "--help"]);
            0
        }
        Some(Commands::Login { email: ref e, password: ref p }) => cmd_login(&cli, &store, base_url, e.clone(), p.clone()),
        Some(Commands::Logout) => cmd_logout(&cli, &store),
        Some(Commands::Status) => cmd_status(&cli, &store, base_url),
        Some(Commands::Feed { ref sub }) => cmd_feed(&cli, &store, base_url, sub),
        Some(Commands::Sync) => {
            eprintln!("Sync from CLI is not yet supported; use the desktop app.");
            1
        }
    }
}

fn cmd_login(
    cli: &Cli,
    store: &FileStore,
    base_url: String,
    email: Option<String>,
    password: Option<String>,
) -> i32 {
    let email = match email {
        Some(e) => e,
        None => {
            eprint!("Email: ");
            let _ = std::io::Write::flush(&mut std::io::stderr());
            let mut s = String::new();
            if std::io::stdin().read_line(&mut s).is_err() || s.trim().is_empty() {
                eprintln!("Email required.");
                return 1;
            }
            s.trim().to_string()
        }
    };
    let password = match password {
        Some(p) => p,
        None => match rpassword::prompt_password("Password: ") {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Password input failed: {}", e);
                return 1;
            }
        },
    };

    let body = LoginRequest {
        email: email.clone(),
        password,
    };
    let api = ApiClient::new(&base_url, None);
    let resp: AuthResponse = match run_async(api.request_with_body(
        "/api/v1/auth/login",
        "POST",
        Some(&body),
        false,
    )) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Login failed: {}", e);
            return 1;
        }
    };

    if let Err(e) = store.set_token(&resp.token) {
        eprintln!("Saved token but config write failed: {}", e);
        return 1;
    }
    if cli.server.is_some() {
        let _ = store.set_server_url(&base_url);
    }

    if cli.json {
        println!("{}", serde_json::json!({ "email": resp.user.email, "plan": resp.user.plan }));
    } else {
        println!("Logged in as {} ({})", resp.user.email, resp.user.plan);
    }
    0
}

fn cmd_logout(cli: &Cli, store: &FileStore) -> i32 {
    if let Err(e) = store.delete_token() {
        eprintln!("Logout failed: {}", e);
        return 1;
    }
    if !cli.json {
        println!("Logged out.");
    }
    0
}

fn cmd_status(cli: &Cli, store: &FileStore, base_url: String) -> i32 {
    let token = match store.get_token() {
        Ok(Some(t)) => t,
        Ok(None) => {
            if cli.json {
                println!("{}", serde_json::json!({ "logged_in": false, "server": base_url }));
            } else {
                println!("Not logged in.");
                println!("Server: {}", base_url);
            }
            return 0;
        }
        Err(e) => {
            eprintln!("Config error: {}", e);
            return 1;
        }
    };

    let api = ApiClient::new(&base_url, Some(token));
    let user: User = match run_async(api.request::<User>("/api/v1/auth/me", "GET", true)) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("Auth check failed: {}", e);
            return 1;
        }
    };

    if cli.json {
        println!(
            "{}",
            serde_json::json!({ "logged_in": true, "email": user.email, "plan": user.plan, "server": base_url })
        );
    } else {
        println!("Logged in as {} ({})", user.email, user.plan);
        println!("Server: {}", base_url);
    }
    0
}

fn cmd_feed(cli: &Cli, store: &FileStore, base_url: String, sub: &FeedSub) -> i32 {
    let token = match store.get_token() {
        Ok(Some(t)) => t,
        Ok(None) => {
            eprintln!("Not logged in. Run 'castify login' first.");
            return 1;
        }
        Err(e) => {
            eprintln!("Config error: {}", e);
            return 1;
        }
    };

    let api = ApiClient::new(&base_url, Some(token));

    match sub {
        FeedSub::List => {
            let feeds: Vec<Feed> = match run_async(api.request::<Vec<Feed>>("/api/v1/feeds", "GET", true)) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Failed to list feeds: {}", e);
                    return 1;
                }
            };
            if cli.json {
                println!("{}", serde_json::to_string(&feeds).unwrap_or_else(|_| "[]".into()));
            } else {
                if feeds.is_empty() {
                    println!("No feeds.");
                } else {
                    println!("{:<40} {:<50}", "ID", "NAME");
                    println!("{}", "-".repeat(92));
                    for f in feeds {
                        println!("{:<40} {:<50}", f.id, f.name);
                    }
                }
            }
            0
        }
        FeedSub::Add {
            ref title,
            ref url,
            ref description,
        } => {
            let body = CreateFeedRequest {
                name: title.clone(),
                source_url: url.clone(),
                description: description.clone(),
            };
            let resp: CreateFeedResponse =
                match run_async(api.request_with_body("/api/v1/feeds", "POST", Some(&body), true)) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to add feed: {}", e);
                        return 1;
                    }
                };
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "id": resp.feed.id, "name": resp.feed.name, "feed_url": resp.feed_url })
                );
            } else {
                println!("Added feed: {} ({})", resp.feed.name, resp.feed.id);
                println!("Feed URL: {}", resp.feed_url);
            }
            0
        }
    }
}
