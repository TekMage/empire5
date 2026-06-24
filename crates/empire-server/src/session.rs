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
// Ported from: src/lib/player/ (player session state machine)
// Known contributors to the original:
//    Dave Pare, 1986, 1994
//    Steve McClure, 1998-2000
//    Ron Koenderink, 2004-2009
//    Markus Armbruster, 2004-2020

// Per-player session handler.
// Replaces src/lib/player/ and the cooperative-thread-per-player model.
//
// Each accepted TCP connection spawns one Tokio task that runs this handler.
// The session progresses through states:
//   Connecting -> Authenticating -> Playing -> Disconnecting
//
// Phase 0 skeleton: handles connect banner and echos commands back.
// Phase 2 will port full login flow and command dispatch.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, info};

use empire_config::Config;
use crate::state::GameState;
use crate::error::ServerResult;
use crate::commands::dispatch;
use crate::protocol::{self, code, MAX_LINE};

/// Session state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    Connecting,
    Authenticating,
    Playing,
    Disconnecting,
}

pub async fn handle(
    socket: TcpStream,
    peer: SocketAddr,
    state: Arc<RwLock<GameState>>,
    config: Arc<Config>,
) -> ServerResult<()> {
    socket.set_nodelay(true)?;
    let (reader, mut writer) = socket.into_split();
    let mut lines = BufReader::new(reader).lines();

    let mut session_state = SessionState::Connecting;

    // --- Connection banner (replaces player_accept + initial handshake)
    // Empire protocol: server sends its version string first.
    let banner = format!(
        "Empire server {} (Rust)\n",
        env!("CARGO_PKG_VERSION")
    );
    writer.write_all(banner.as_bytes()).await?;

    // Send the login prompt code
    writer
        .write_all(protocol::response(code::PROMPT, "Empire Server, login:").as_bytes())
        .await?;

    session_state = SessionState::Authenticating;

    // Country number for this session (set after auth, Phase 2)
    let mut cnum: Option<u8> = None;

    // --- Main input loop
    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_LINE {
            writer
                .write_all(
                    protocol::response(code::BADARG, "line too long").as_bytes(),
                )
                .await?;
            continue;
        }

        debug!(%peer, cmd = %line, "Received");

        match session_state {
            SessionState::Authenticating => {
                // Phase 2 will implement full login: "login <country> <password>"
                // For now, accept any "login" line and move to Playing.
                if line.starts_with("login") {
                    info!(%peer, "Authenticated (stub)");
                    cnum = Some(1);
                    session_state = SessionState::Playing;
                    writer
                        .write_all(
                            protocol::response(code::OK, "Welcome to Empire 5 (dev build)").as_bytes(),
                        )
                        .await?;
                } else if line.starts_with("client") {
                    // client identification, ignore for now
                } else {
                    writer
                        .write_all(
                            protocol::response(code::BADCOUNTRY, "login required").as_bytes(),
                        )
                        .await?;
                }
            }

            SessionState::Playing => {
                if line == "quit" || line == "bye" {
                    session_state = SessionState::Disconnecting;
                    break;
                }
                // Dispatch command (Phase 5 will fill this in fully)
                let gs = state.read().await;
                let output = dispatch(&line, cnum.unwrap_or(0), &gs).await;
                drop(gs);
                writer.write_all(output.as_bytes()).await?;
                // Send prompt ready for next command
                writer
                    .write_all(
                        protocol::response(code::PROMPT, "Command:").as_bytes(),
                    )
                    .await?;
            }

            _ => break,
        }
    }

    info!(%peer, "Connection closed");
    let _ = writer
        .write_all(protocol::response(code::EXIT, "Goodbye").as_bytes())
        .await;
    Ok(())
}
