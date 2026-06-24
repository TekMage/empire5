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
// Ported from: src/lib/commands/vers.c
// Known contributors to the original:
//    Dave Pare
//    Jeff Bailey
//    Thomas Ruschak
//    Ken Stevens
//    Steve McClure

// ref: src/lib/commands/vers.c
//
// "version" command — report server version and game parameters.
// Phase 0: reports Rust version string only.  Full output (tech, research,
// mobility scales, etc.) added in Phase 5 when GameConfig is wired up.

use crate::state::GameState;

pub async fn run(_args: &str, _cnum: u8, _state: &GameState) -> String {
    format!(
        "2 Empire 5 version {ver} (Rust rewrite of Wolfpack Empire)\n\
         2 Protocol: original Empire text protocol\n\
         2 Build: {profile}\n\
         1 version\n",
        ver = env!("CARGO_PKG_VERSION"),
        profile = if cfg!(debug_assertions) { "debug" } else { "release" },
    )
}
