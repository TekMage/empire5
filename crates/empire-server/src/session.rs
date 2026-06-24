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
//
// Each accepted TCP connection spawns one Tokio task running handle().
// The session progresses:
//
//   Connecting → Authenticating → Playing → Disconnecting
//
// Pre-login commands (from src/lib/player/login.c):
//   client <id...>         — record client identification
//   user <name>            — set Unix userid
//   coun <country-name>    — look up cnum by name
//   pass <password>        — verify password for current cnum
//   play [user [coun [pw]]]— all-in-one: set user+coun+pw, enter Playing
//   options [key=val...]   — set session options (utf-8)
//   kill                   — disconnect a duplicate session for same cnum
//   quit                   — close connection
//
// After successful play:
//   Server sends C_INIT with protocol version (CLIENTPROTO = 2)
//   Shows MOTD / last-login info, then enters the command loop.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use empire_config::Config;
use empire_db::nations;

use crate::journal::Journal;
use crate::state::{GameState, SessionInfo, SessionRegistry};
use crate::error::ServerResult;
use crate::commands::dispatch;
use crate::protocol::{self, code, CLIENT_PROTO, MAX_LINE};

/// Per-session mutable state accumulated during the pre-login phase.
struct LoginCtx {
    cnum: Option<u8>,
    authenticated: bool,
    user_id: String,
    client_id: String,
    utf8: bool,
}

impl LoginCtx {
    fn new() -> Self {
        LoginCtx {
            cnum: None,
            authenticated: false,
            user_id: String::new(),
            client_id: String::new(),
            utf8: false,
        }
    }
}

pub async fn handle(
    socket: TcpStream,
    peer: SocketAddr,
    state: Arc<RwLock<GameState>>,
    sessions: Arc<SessionRegistry>,
    journal: Arc<Journal>,
    config: Arc<Config>,
    conn_id: u64,
) -> ServerResult<()> {
    socket.set_nodelay(true)?;
    let (reader, mut writer) = socket.into_split();
    let mut lines = BufReader::new(reader).lines();

    let host_addr = peer.ip().to_string();
    let thread_name = format!("Session-{conn_id}");

    // C_INIT greeting — "Empire server ready"
    // Mirrors: pr_id(player, C_INIT, "Empire server ready\n") in player_login()
    writer.write_all(
        protocol::response(code::INIT, "Empire server ready").as_bytes()
    ).await?;

    let mut ctx = LoginCtx::new();

    // ── Pre-login command loop ────────────────────────────────────────────────
    loop {
        let raw = match lines.next_line().await? {
            Some(l) => l,
            None => {
                // Client disconnected before logging in
                return Ok(());
            }
        };
        let line = raw.trim().to_string();
        if line.is_empty() { continue; }
        if line.len() > MAX_LINE {
            send(&mut writer, code::BADCMD, "line too long").await?;
            continue;
        }

        debug!(%peer, cmd = %line, "pre-login");
        journal.input(&thread_name, &line);

        let parts: Vec<&str> = line.splitn(10, ' ').collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "client" => {
                // client <id...>  — record client identification string
                ctx.client_id = parts[1..].join(" ");
                send(&mut writer, code::CMDOK,
                     &format!("talking to {}", ctx.client_id)).await?;
            }

            "user" => {
                // user <name>  — set Unix userid
                match parts.get(1) {
                    Some(name) => {
                        ctx.user_id = name.to_string();
                        send(&mut writer, code::CMDOK,
                             &format!("hello {}", ctx.user_id)).await?;
                    }
                    None => send(&mut writer, code::BADCMD,
                                "Usage: user <name>").await?,
                }
            }

            "coun" => {
                // coun <country-name>  — look up cnum, clear authenticated
                match parts.get(1) {
                    Some(name) => {
                        let gs = state.read().await;
                        let found = nations::natbyname(&gs.db, name).await;
                        drop(gs);
                        match found {
                            Ok(Some(nat)) => {
                                ctx.cnum = Some(nat.cnum);
                                ctx.authenticated = false;
                                send(&mut writer, code::CMDOK,
                                     &format!("country name {name}")).await?;
                            }
                            Ok(None) => {
                                send(&mut writer, code::CMDERR,
                                     &format!("country {name} does not exist")).await?;
                            }
                            Err(e) => {
                                warn!(error=%e, "natbyname db error");
                                send(&mut writer, code::CMDERR, "database error").await?;
                            }
                        }
                    }
                    None => send(&mut writer, code::BADCMD,
                                "Usage: coun <country>").await?,
                }
            }

            "pass" => {
                // pass <password>  — verify password for current cnum
                match (ctx.cnum, parts.get(1).copied()) {
                    (None, _) => {
                        send(&mut writer, code::CMDERR, "need country first").await?;
                    }
                    (Some(_), None) => {
                        send(&mut writer, code::BADCMD, "Usage: pass <password>").await?;
                    }
                    (Some(cnum), Some(pw)) => {
                        let gs = state.read().await;
                        let ok = nations::verify_passwd(&gs.db, cnum, pw).await;
                        drop(gs);
                        match ok {
                            Ok(true) => {
                                ctx.authenticated = true;
                                send(&mut writer, code::CMDOK, "password ok").await?;
                            }
                            Ok(false) => {
                                warn!(%peer, cnum, "bad password");
                                send(&mut writer, code::CMDERR,
                                     "password bad, logging entry").await?;
                            }
                            Err(e) => {
                                warn!(error=%e, "verify_passwd db error");
                                send(&mut writer, code::CMDERR, "database error").await?;
                            }
                        }
                    }
                }
            }

            "play" => {
                // play [user [country [password]]]
                // All-in-one login; then transitions to Playing state.
                let mut ap = parts.iter().skip(1).copied();

                if let Some(u) = ap.next() { ctx.user_id = u.to_string(); }

                if let Some(cname) = ap.next() {
                    let gs = state.read().await;
                    let found = nations::natbyname(&gs.db, cname).await;
                    drop(gs);
                    match found {
                        Ok(Some(nat)) => { ctx.cnum = Some(nat.cnum); }
                        Ok(None) => {
                            send(&mut writer, code::CMDERR,
                                 &format!("country {cname} does not exist")).await?;
                            continue;
                        }
                        Err(e) => {
                            warn!(error=%e, "natbyname db error");
                            send(&mut writer, code::CMDERR, "database error").await?;
                            continue;
                        }
                    }
                }

                if let Some(pw) = ap.next() {
                    match ctx.cnum {
                        None => {
                            send(&mut writer, code::CMDERR, "need country first").await?;
                            continue;
                        }
                        Some(cnum) => {
                            let gs = state.read().await;
                            let ok = nations::verify_passwd(&gs.db, cnum, pw).await;
                            drop(gs);
                            match ok {
                                Ok(true) => { ctx.authenticated = true; }
                                Ok(false) => {
                                    warn!(%peer, cnum, "bad password in play");
                                    send(&mut writer, code::CMDERR,
                                         "password bad, logging entry").await?;
                                    continue;
                                }
                                Err(e) => {
                                    warn!(error=%e, "verify_passwd db error");
                                    send(&mut writer, code::CMDERR, "database error").await?;
                                    continue;
                                }
                            }
                        }
                    }
                }

                // May play?
                match ctx.cnum {
                    None => {
                        send(&mut writer, code::CMDERR,
                             "need country and password").await?;
                        continue;
                    }
                    Some(cnum) if !ctx.authenticated => {
                        send(&mut writer, code::CMDERR,
                             &format!("need country and password (cnum={cnum})")).await?;
                        continue;
                    }
                    Some(cnum) => {
                        // Check if cnum is already in a Playing session
                        if let Some(other) = sessions.get(cnum) {
                            send(&mut writer, code::EXIT,
                                 &format!("country in use by {}@{}",
                                          other.user_id, other.host_addr)).await?;
                            return Ok(());
                        }

                        // Register this session
                        let info = SessionInfo {
                            cnum,
                            host_addr: host_addr.clone(),
                            user_id: ctx.user_id.clone(),
                            thread_name: thread_name.clone(),
                        };
                        if !sessions.try_enter(info) {
                            send(&mut writer, code::EXIT, "country in use").await?;
                            return Ok(());
                        }

                        // Protocol handshake — C_INIT with protocol version
                        // Mirrors: pr_id(player, C_INIT, "%d\n", CLIENTPROTO)
                        writer.write_all(
                            protocol::response(code::INIT, &CLIENT_PROTO.to_string()).as_bytes()
                        ).await?;

                        // Persist login metadata
                        let now = unix_now();
                        {
                            let gs = state.read().await;
                            let _ = nations::record_login(
                                &gs.db, cnum, &host_addr, &ctx.user_id, now
                            ).await;
                        }

                        info!(%peer, cnum, user_id = %ctx.user_id,
                              "Logged in");
                        journal.login(&thread_name, cnum, &host_addr, &ctx.user_id);

                        // Enter Playing state
                        let result = playing_loop(
                            &mut lines, &mut writer,
                            peer, cnum, &ctx.user_id, &host_addr,
                            &thread_name,
                            &state, &sessions, &journal, &config,
                        ).await;

                        // Cleanup after session ends
                        let now = unix_now();
                        {
                            let gs = state.read().await;
                            let _ = nations::record_logout(&gs.db, cnum, now).await;
                        }
                        journal.logout(&thread_name, cnum);
                        info!(%peer, cnum, "Logged out");
                        sessions.leave(cnum);

                        let _ = writer.write_all(
                            protocol::response(code::EXIT, "Goodbye").as_bytes()
                        ).await;

                        return result;
                    }
                }
            }

            "options" => {
                // options [key=val...]  — session flags
                if parts.len() == 1 {
                    // List current options
                    writer.write_all(
                        protocol::data_line(&format!("utf-8={}", ctx.utf8 as u8)).as_bytes()
                    ).await?;
                    send(&mut writer, code::CMDOK, "").await?;
                } else {
                    for kv in &parts[1..] {
                        if let Some((k, v)) = kv.split_once('=') {
                            match k {
                                "utf-8" => ctx.utf8 = v != "0",
                                _ => {
                                    send(&mut writer, code::BADCMD,
                                         &format!("Option {k} not found")).await?;
                                    // per C source: return RET_FAIL on unknown option
                                    // but we continue here for simplicity
                                }
                            }
                        }
                    }
                    send(&mut writer, code::CMDOK, "Accepted").await?;
                }
            }

            "kill" => {
                // kill  — disconnect duplicate session for same cnum
                match ctx.cnum {
                    None => send(&mut writer, code::CMDERR,
                                "need country and password").await?,
                    Some(_) if !ctx.authenticated =>
                        send(&mut writer, code::CMDERR,
                             "need country and password").await?,
                    Some(cnum) => {
                        match sessions.get(cnum) {
                            None => {
                                send(&mut writer, code::EXIT,
                                     "country not in use").await?;
                                return Ok(());
                            }
                            Some(other) => {
                                // Mark the other session for shutdown via registry eviction.
                                // The other task will notice its cnum is gone on next
                                // status check (Phase 3 will add a proper abort channel).
                                sessions.leave(cnum);
                                send(&mut writer, code::EXIT,
                                     &format!("terminated {}@{}'s connection",
                                              other.user_id, other.host_addr)).await?;
                                return Ok(());
                            }
                        }
                    }
                }
            }

            "quit" => {
                let _ = writer.write_all(
                    protocol::response(code::EXIT, "so long...").as_bytes()
                ).await;
                return Ok(());
            }

            _ => {
                send(&mut writer, code::BADCMD,
                     &format!("Command {} not found", parts[0])).await?;
            }
        }
    }
}

// ── Playing state command loop ────────────────────────────────────────────────

async fn playing_loop<R>(
    lines: &mut tokio::io::Lines<tokio::io::BufReader<R>>,
    writer: &mut (impl AsyncWriteExt + Unpin),
    peer: SocketAddr,
    cnum: u8,
    _user_id: &str,
    _host_addr: &str,
    thread_name: &str,
    state: &Arc<RwLock<GameState>>,
    sessions: &Arc<SessionRegistry>,
    journal: &Arc<Journal>,
    _config: &Arc<Config>,
) -> ServerResult<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    // Show a brief MOTD / last-login notice as C_DATA lines
    // (Phase 5 will load the real MOTD file; this is a placeholder)
    {
        let gs = state.read().await;
        if let Ok(Some(nat)) = empire_db::nations::get_by_cnum(&gs.db, cnum).await {
            if nat.last_login > 0 {
                use chrono::{DateTime, Utc, TimeZone};
                let dt: DateTime<Utc> = Utc.timestamp_opt(nat.last_login, 0)
                    .single().unwrap_or_else(Utc::now);
                writer.write_all(
                    protocol::data_line(
                        &format!("Last connection from: {} at {}",
                                 nat.host_addr, dt.format("%a %b %e %T %Y"))
                    ).as_bytes()
                ).await?;
            }
            // Telegram notification (NF_INFORM handled in Phase 5)
            if nat.tele_cnt > 0 {
                let msg = if nat.tele_cnt == 1 {
                    "You have a new telegram waiting ...".to_string()
                } else {
                    format!("You have {} new telegrams waiting ...", nat.tele_cnt)
                };
                writer.write_all(protocol::data_line(&msg).as_bytes()).await?;
            }
        }
    }

    // Initial prompt
    writer.write_all(
        protocol::response(code::PROMPT, "Command:").as_bytes()
    ).await?;

    // Command loop — mirrors player_main() / command() in player.c
    loop {
        let raw = match lines.next_line().await? {
            Some(l) => l,
            None => break, // TCP closed
        };
        let line = raw.trim().to_string();
        if line.is_empty() { continue; }
        if line.len() > MAX_LINE {
            writer.write_all(
                protocol::response(code::BADCMD, "line too long").as_bytes()
            ).await?;
            continue;
        }

        debug!(%peer, cnum, cmd = %line, "command");
        journal.input(thread_name, &line);

        let cmd_name = line.split_whitespace().next().unwrap_or("");

        // Verify the session hasn't been killed via the registry
        if sessions.get(cnum).is_none() {
            writer.write_all(
                protocol::data_line("Disconnected by another session").as_bytes()
            ).await?;
            break;
        }

        match cmd_name.to_lowercase().as_str() {
            "quit" | "bye" => break,

            _ => {
                journal.command(thread_name, cmd_name);
                let gs = state.read().await;
                let output = dispatch(&line, cnum, &gs).await;
                drop(gs);
                writer.write_all(output.as_bytes()).await?;
                writer.write_all(
                    protocol::response(code::PROMPT, "Command:").as_bytes()
                ).await?;
            }
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn send(
    writer: &mut (impl AsyncWriteExt + Unpin),
    code: &str,
    msg: &str,
) -> ServerResult<()> {
    writer.write_all(protocol::response(code, msg).as_bytes()).await?;
    Ok(())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
