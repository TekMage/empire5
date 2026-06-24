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
// Ported from: src/server/xdump.c, src/lib/commands/xdump.c

// xdump command — dump game state as text.
// Usage: xdump <type> [area] [?cond ...]

use super::ctx::CmdCtx;
use empire_types::selector::parse_scan_spec;
use empire_db::{scan, xdump};
use empire_db::scan::ScanResult;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let input = if args.is_empty() { "sect *" } else { args };

    let spec = match parse_scan_spec(input) {
        Ok(s) => s,
        Err(e) => return format!("10 {e}\n"),
    };

    let result = match scan::scan(ctx.db, &spec).await {
        Ok(r) => r,
        Err(e) => return format!("10 database error: {e}\n"),
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

    let mut out = String::new();
    for line in dump.lines() {
        out.push_str(&format!("2 {line}\n"));
    }
    out.push_str("0 xdump\n");
    out
}
