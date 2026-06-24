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
// Ported from: src/lib/commands/info.c

// "info <topic>" command — display help text.
// Phase 5: placeholder; full info-page serving deferred to Phase 6.

use super::ctx::CmdCtx;

pub async fn run(topic: &str, _ctx: &CmdCtx<'_>) -> String {
    if topic.is_empty() {
        "2 Info topics not yet available (Phase 6).\n\
         2 Use the C reference server's info pages for now.\n\
         1 info\n"
            .to_string()
    } else {
        format!(
            "2 Info on '{topic}' not yet available (Phase 6).\n\
             1 info\n"
        )
    }
}
