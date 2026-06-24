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
//
// Ported from: src/lib/commands/vers.c

// "version" command — report server version and game parameters.

use super::ctx::CmdCtx;

pub async fn run(_args: &str, ctx: &CmdCtx<'_>) -> String {
    format!(
        "2 Empire 5 version {ver} (Rust rewrite of Wolfpack Empire)\n\
         2 Protocol: original Empire text protocol\n\
         2 Build: {profile}\n\
         2 World: {wx} x {wy}  ETU: {etu}\n\
         1 version\n",
        ver     = env!("CARGO_PKG_VERSION"),
        profile = if cfg!(debug_assertions) { "debug" } else { "release" },
        wx      = ctx.world_x,
        wy      = ctx.world_y,
        etu     = ctx.etu,
    )
}
