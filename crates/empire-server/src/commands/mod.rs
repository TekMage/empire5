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

// Command dispatch table.
// Replaces src/lib/commands/ (151 .c files) and the C cmndstr dispatch table.
//
// Phase 0: minimal stub commands to prove the dispatch mechanism.
// Phase 5: all 151 commands fully ported.

use crate::state::GameState;
use crate::protocol::{code, response};

mod version;
mod info;
mod xdump;

/// Dispatch a command line to the appropriate handler.
/// Returns the complete server response string (data + final code line).
pub async fn dispatch(line: &str, cnum: u8, state: &GameState) -> String {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).copied().unwrap_or("");

    match cmd.as_str() {
        "version" | "vers" => version::run(args, cnum, state).await,
        "info"             => info::run(args, cnum, state).await,
        "echo"             => echo_cmd(args),
        "xdump"            => xdump::run(args, cnum, state).await,

        _ => response(code::BADCMD, &format!("Unknown command: {cmd}")),
    }
}

fn echo_cmd(args: &str) -> String {
    format!("{} {args}\n{} echo\n", code::INIT, code::DATA)
}
