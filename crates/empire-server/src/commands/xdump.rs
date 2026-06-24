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
// Ported from: src/server/xdump.c, src/lib/commands/xdump.c
// Known contributors to the original:
//    Markus Armbruster, 2004-2016

// xdump command — dumps game state as text.
// Usage: xdump <type> [area] [?cond ...]
// ref: src/server/xdump.c (empire4.4.1)

use crate::state::GameState;
use empire_types::selector::parse_scan_spec;
use empire_db::{scan, xdump};
use empire_db::scan::ScanResult;

pub async fn run(args: &str, _cnum: u8, state: &GameState) -> String {
    let input = if args.is_empty() { "sect *" } else { args };

    let spec = match parse_scan_spec(input) {
        Ok(s) => s,
        Err(e) => return format!("421 {e}\n"),
    };

    let result = match scan::scan(&state.db, &spec).await {
        Ok(r) => r,
        Err(e) => return format!("421 database error: {e}\n"),
    };

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let dump = match result {
        ScanResult::Nations(v)   => xdump::dump_nations(&v, ts),
        ScanResult::Sectors(v)   => xdump::dump_sectors(&v, ts),
        ScanResult::Ships(v)     => xdump::dump_ships(&v, ts),
        ScanResult::Planes(v)    => xdump::dump_planes(&v, ts),
        ScanResult::LandUnits(v) => xdump::dump_land_units(&v, ts),
        ScanResult::Nukes(v)     => xdump::dump_nukes(&v, ts),
    };

    // Stream each data line with code "2", then close with "0 xdump\n".
    let mut out = String::new();
    for line in dump.lines() {
        out.push_str(&format!("2 {line}\n"));
    }
    out.push_str("0 xdump\n");
    out
}
