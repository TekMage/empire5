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
// Every server response line begins with a decimal numeric code, matching the
// C constants in include/proto.h:
//
//   C_CMDOK  = 0x0  success, no further output
//   C_DATA   = 0x1  data line (game output)
//   C_INIT   = 0x2  connection / play start confirmation
//   C_EXIT   = 0x3  server closing connection
//   C_FLUSH  = 0x4  flush buffered output
//   C_NOECHO = 0x5  suppress client echo (for password input)
//   C_PROMPT = 0x6  ready for next command
//   C_ABORT  = 0x7  command aborted
//   C_CMDERR = 0xA  command execution error
//   C_BADCMD = 0xB  unknown command
//
// Client → server login commands (before play):
//   client <id...>
//   user <name>
//   coun <country-name>
//   pass <password>
//   play [user [country [password]]]
//   options [key=val...]
//   kill
//   quit
//
// After successful play the server sends C_INIT with the protocol version:
//   "2 2\n"  (CLIENTPROTO = 2)

/// Protocol codes — decimal string representation sent on the wire.
/// Values match the C_* constants in include/proto.h.
pub mod code {
    pub const CMDOK:  &str = "0";   // C_CMDOK  — success
    pub const DATA:   &str = "1";   // C_DATA   — data line
    pub const INIT:   &str = "2";   // C_INIT   — connection / play start
    pub const EXIT:   &str = "3";   // C_EXIT   — closing connection
    pub const FLUSH:  &str = "4";   // C_FLUSH  — flush output
    pub const NOECHO: &str = "5";   // C_NOECHO — suppress client echo
    pub const PROMPT: &str = "6";   // C_PROMPT — ready for command
    pub const ABORT:  &str = "7";   // C_ABORT  — command aborted
    pub const CMDERR: &str = "10";  // C_CMDERR — command error (0xA)
    pub const BADCMD: &str = "11";  // C_BADCMD — unknown command (0xB)
}

/// Protocol version sent to the client after a successful play command.
/// Matches CLIENTPROTO in include/proto.h.
pub const CLIENT_PROTO: u32 = 2;

/// Maximum line length accepted from a client (bytes).
pub const MAX_LINE: usize = 1024;

/// Format a server response line: "<code> <message>\n"
pub fn response(code: &str, message: &str) -> String {
    format!("{code} {message}\n")
}

/// Format a data line: "1 <text>\n"
pub fn data_line(text: &str) -> String {
    format!("{} {text}\n", code::DATA)
}
