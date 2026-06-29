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
// Ported from: src/lib/player/player.c  (c_execute)

// "execute" command — run a local client-side script file.
//
// Usage: execute <filename>
//   execute /path/to/script.emp
//
// This sends a C_EXECUTE protocol message (id=12) to the client with the
// given filename.  The client opens that file locally and feeds each line
// back to the server as commands, allowing scripted sessions.
//
// This is the standard Empire mechanism for batch operations.  Write a
// script on the client machine, connect to the server, and run:
//   execute /Users/me/scripts/my_script.emp
//
// Protocol: the server sends "12 <filename>\n" (C_EXECUTE, id=0xC=12).
// The client (empire 4.4.1) opens the file and switches input to it.
// When the file is exhausted, the client returns to normal stdin input.
// Each command from the file is processed by the server's normal dispatch loop.

use super::ctx::CmdCtx;

pub async fn run(args: &str, _ctx: &CmdCtx<'_>) -> String {
    let filename = args.trim();
    if filename.is_empty() {
        return "10 Usage: execute <filename>\n".to_string();
    }
    // C_EXECUTE = protocol id 12 (0xC).
    // The client will open `filename` on the client machine and feed its
    // lines back as commands.  No "0 execute\n" terminator is sent here
    // because the execute session continues until the file is exhausted.
    format!("12 {filename}\n0 execute\n")
}
