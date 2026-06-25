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
// Ported from: src/lib/commands/powe.c

// "power" command — display a power ranking of all active nations.
//
// Power formula:
//   tech_factor = 1.0 + nation.tech * 0.01
//   power = (owned_sectors * 1000 + total_civs) * tech_factor

use empire_db::{nations, sectors};
use empire_types::commodity::Item;
use super::ctx::CmdCtx;

struct PowerEntry {
    cnum: u8,
    name: String,
    owned_sectors: u32,
    total_civs: i64,
    tech: f64,
    power: f64,
}

pub async fn run(_args: &str, ctx: &CmdCtx<'_>) -> String {
    let all_nations = match nations::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };
    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // Count sectors and civilians per nation
    let mut sect_count = vec![0u32; 100];
    let mut civ_count  = vec![0i64; 100];

    for s in &all_sectors {
        let own = s.own as usize;
        if own == 0 || own >= 100 {
            continue;
        }
        sect_count[own] += 1;
        civ_count[own]  += s.items.get(Item::Civil) as i64;
    }

    let mut entries: Vec<PowerEntry> = all_nations.iter()
        .filter(|n| n.status.is_active())
        .map(|n| {
            let own = n.cnum as usize;
            let owned = sect_count.get(own).copied().unwrap_or(0);
            let civs  = civ_count.get(own).copied().unwrap_or(0);
            let tech_factor = 1.0 + n.tech * 0.01;
            let power = (owned as f64 * 1000.0 + civs as f64) * tech_factor;
            PowerEntry {
                cnum: n.cnum,
                name: n.name.clone(),
                owned_sectors: owned,
                total_civs: civs,
                tech: n.tech,
                power,
            }
        })
        .collect();

    // Sort by power descending
    entries.sort_by(|a, b| b.power.partial_cmp(&a.power).unwrap_or(std::cmp::Ordering::Equal));

    let mut out = String::new();
    out.push_str("1 POWER                          sectors  civs      tech   power\n");
    for e in &entries {
        out.push_str(&format!(
            "1 {:25} ({:2})  {:5}  {:8}  {:6.2}  {:8.0}\n",
            e.name, e.cnum, e.owned_sectors, e.total_civs, e.tech, e.power
        ));
    }
    if entries.is_empty() {
        out.push_str("1 No active nations\n");
    }
    out.push_str("0 power\n");
    out
}
