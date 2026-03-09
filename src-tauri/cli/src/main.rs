//! CLI-only binary for power users / AI agents.
//! Build with: cargo build --release -p castify-cli
//! No Tauri or GUI dependencies; minimal footprint.

fn main() {
    let code = castify::cli::run(std::env::args());
    std::process::exit(code);
}
