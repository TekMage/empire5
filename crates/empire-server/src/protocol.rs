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

// Empire wire protocol helpers.
//
// The protocol is a simple line-oriented text protocol over TCP (port 6665).
// Server responses begin with a numeric code prefix (like SMTP/FTP):
//
//   "version <N>\n"   — sent at connect
//   "login <country> <password>\n"  — client auth
//   "client <name> <version>\n"     — client identification (optional)
//   "<command> [args]\n"            — player commands
//
// Server sends:
//   "<data lines>"
//   "<code> <message>\n"  — code 0xx = success, 4xx = failure
//
// This module provides line-reading/writing helpers.  Full protocol
// implementation happens in Phase 2.

/// Protocol response codes (matches empire4.x C/S_* constants)
#[allow(dead_code)]
pub mod code {
    pub const OK: &str = "0";          // C_OK
    pub const CMDOK: &str = "1";       // C_CMDOK — command accepted, more output follows
    pub const DATA: &str = "2";        // C_DATA  — data line
    pub const PROMPT: &str = "6";      // C_PROMPT — ready for next command
    pub const FLUSH: &str = "7";       // C_FLUSH — flush output
    pub const EXIT: &str = "12";       // C_EXIT  — server is closing connection
    pub const BADCMD: &str = "421";    // C_BADCMD
    pub const BADARG: &str = "422";    // C_BADARG
    pub const CMDERR: &str = "424";    // C_CMDERR
    pub const NOPLAY: &str = "500";    // C_NOPLAY — can't play right now
    pub const BADCOUNTRY: &str = "501"; // C_BADCOUNTRY
    pub const BADPASS: &str = "502";   // C_BADPASS
}

/// Maximum line length accepted from a client (bytes).
pub const MAX_LINE: usize = 1024;

/// Format a server response line.
pub fn response(code: &str, message: &str) -> String {
    format!("{code} {message}\n")
}

/// Format a data line (raw game output, no code prefix in data body).
pub fn data_line(text: &str) -> String {
    format!("{} {text}\n", code::DATA)
}
