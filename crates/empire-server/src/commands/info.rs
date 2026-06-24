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
// Ported from: src/lib/commands/info.c
// Known contributors to the original:
//    Dave Pare, 1986
//    Mike Wise, 1997
//    Doug Hay, 1998
//    Steve McClure, 1998-2000
//    Ron Koenderink, 2004

// ref: src/lib/commands/info.c
//
// "info <topic>" command — display help text.
// Phase 0: returns a placeholder.  Phase 5 will serve the info pages.

use crate::state::GameState;

pub async fn run(topic: &str, _cnum: u8, _state: &GameState) -> String {
    if topic.is_empty() {
        "2 Info topics not yet available (Phase 5).\n\
         2 Use the C reference server's info pages for now.\n\
         1 info\n"
            .to_string()
    } else {
        format!(
            "2 Info on '{topic}' not yet available (Phase 5).\n\
             1 info\n"
        )
    }
}
