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
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.

// Command dispatch table.
// Replaces src/lib/commands/ (151 .c files) and the C cmndstr dispatch table.
//
// Phase 5: core info and economic commands implemented.

pub mod ctx;
mod version;
mod info;
mod xdump;
mod census;
mod nation_cmd;
mod map_cmd;
mod designate;
mod threshold;
mod relations_cmd;
mod declare;

use crate::state::GameState;
use crate::protocol::{code, response};
use empire_config::Config;

/// Dispatch a command line to the appropriate handler.
/// Loads the issuing nation from DB and builds a `CmdCtx` once per command.
pub async fn dispatch(line: &str, cnum: u8, state: &GameState, cfg: &Config) -> String {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd  = parts[0].to_lowercase();
    let args = parts.get(1).copied().unwrap_or("");

    // Load nation record — required for coordinate transforms and identity checks
    let nat = match empire_db::nations::get_by_cnum(&state.db, cnum).await {
        Ok(Some(n)) => n,
        Ok(None)    => return response(code::CMDERR, "Internal error: nation not found"),
        Err(e)      => return response(code::CMDERR, &format!("DB error: {e}")),
    };

    let is_deity = nat.is_deity();

    let ctx = ctx::CmdCtx {
        cnum,
        nat,
        is_deity,
        db: &state.db,
        world_x: cfg.game.world_x,
        world_y: cfg.game.world_y,
        etu: cfg.game.etu_per_update,
    };

    match cmd.as_str() {
        "version" | "vers"          => version::run(args, &ctx).await,
        "info"                      => info::run(args, &ctx).await,
        "echo"                      => echo_cmd(args),
        "xdump"                     => xdump::run(args, &ctx).await,

        "census" | "cens"           => census::run(args, &ctx).await,
        "nation" | "nati"           => nation_cmd::run(args, &ctx).await,
        "map"                       => map_cmd::run(args, &ctx).await,
        "bmap"                      => map_cmd::run(args, &ctx).await,
        "smap" | "sector" | "sect"  => map_cmd::run(args, &ctx).await,
        "designate" | "desi"        => designate::run(args, &ctx).await,
        "threshold" | "thre"        => threshold::run(args, &ctx).await,
        "relations" | "rela"        => relations_cmd::run(args, &ctx).await,
        "declare"   | "decl"        => declare::run(args, &ctx).await,

        _ => response(code::BADCMD, &format!("Unknown command: {cmd}")),
    }
}

fn echo_cmd(args: &str) -> String {
    format!("{} {args}\n{} echo\n", code::INIT, code::DATA)
}
