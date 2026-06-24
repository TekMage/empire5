// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// Empire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/server/main.c
// Known contributors to the original:
//    Dave Pare, 1994
//    Steve McClure, 1996, 1998
//    Doug Hay, 1998
//    Ron Koenderink, 2004-2009
//    Markus Armbruster, 2005-2017

// Empire 5 — async game server entry point.
// Replaces src/server/main.c from empire4.x.
//
// Architecture:
//   - One Tokio task per player connection (replaces cooperative LWP threads)
//   - An update task fires on a configurable interval (replaces SIGALRM + empth_create)
//   - A shared RwLock<GameState> serializes update vs. player commands

mod session;
mod commands;
mod update;
mod state;
mod error;
mod protocol;
mod journal;

use std::path::PathBuf;
use std::sync::Arc;
use clap::Parser;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;

use empire_config::{Config, load_or_default};
use empire_db::Db;
use journal::Journal;
use state::{GameState, SessionRegistry};

#[derive(Parser, Debug)]
#[command(name = "empire-server", about = "Empire 5 game server")]
struct Args {
    /// Path to the TOML configuration file
    #[arg(short, long, default_value = "config/empire.toml")]
    config: PathBuf,

    /// Override the TCP port from config
    #[arg(short, long)]
    port: Option<u16>,

    /// Enable debug logging (sets RUST_LOG=debug if not already set)
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize structured logging (replaces loginit/logerror from log.c)
    let filter = if args.debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Load configuration
    let mut config: Config = load_or_default(&args.config);
    if let Some(port) = args.port {
        config.server.port = port;
    }

    info!(
        version = env!("CARGO_PKG_VERSION"),
        port = config.server.port,
        data_dir = %config.server.data_dir.display(),
        "Empire 5 server starting"
    );

    // Open database (creates and migrates if new)
    let db_path = config.server.data_dir.join("empire.db");
    std::fs::create_dir_all(&config.server.data_dir)?;
    let db = Db::open(&db_path).await?;
    info!(path = %db_path.display(), "Database ready");

    // Open the journal (append-only event log at data/journal)
    let journal = Arc::new(Journal::open(&config.server.data_dir)?);
    journal.startup();
    info!(path = %config.server.data_dir.join("journal").display(), "Journal open");

    // Session registry — tracks which cnums are currently playing
    let sessions: Arc<SessionRegistry> = Arc::new(SessionRegistry::new());

    // Shared game state — wrapped in Arc<RwLock> so the update task can
    // take an exclusive write lock while player tasks hold shared read locks.
    let state = Arc::new(RwLock::new(GameState::new(db.clone())));

    // Spawn the update engine
    let update_state = Arc::clone(&state);
    let update_cfg = config.update.clone();
    let update_journal = Arc::clone(&journal);
    let update_config = Arc::new(config.clone());
    tokio::spawn(async move {
        update::run_update_loop(update_state, update_cfg, update_journal, update_config).await;
    });

    // Bind TCP listener (replaces tcp_listen.c + player_accept thread)
    let host = if config.server.listen_addr.is_empty() {
        "0.0.0.0"
    } else {
        &config.server.listen_addr
    };
    let addr = format!("{host}:{}", config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    info!(addr, "Listening for player connections");

    // Accept loop — one task per connection
    let config = Arc::new(config);
    let mut conn_id: u64 = 0;
    loop {
        match listener.accept().await {
            Ok((socket, peer_addr)) => {
                conn_id += 1;
                info!(%peer_addr, conn_id, "New connection");
                let state = Arc::clone(&state);
                let sessions = Arc::clone(&sessions);
                let journal = Arc::clone(&journal);
                let cfg = Arc::clone(&config);
                tokio::spawn(async move {
                    if let Err(e) = session::handle(
                        socket, peer_addr, state, sessions, journal, cfg, conn_id
                    ).await {
                        warn!(%peer_addr, conn_id, error = %e, "Session ended with error");
                    }
                });
            }
            Err(e) => {
                error!(error = %e, "Accept failed");
            }
        }
    }
}
