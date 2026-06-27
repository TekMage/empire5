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
// Ported from: src/lib/commands/capital.c

// "capital" command — move a nation's capital sector.
// "newcap" command — deity creates/assigns a new capital location.
//
// capital X,Y          — player moves own capital to owned urban/mountain sector
// capital N X,Y        — deity moves nation N's capital
// newcap N X,Y         — deity assigns a new capital (creates urban if needed)

use empire_db::{nations, sectors};
use empire_types::coords::Coord;
use empire_types::commodity::{Inventory, Item};
use empire_types::sector::SectorType;
use super::ctx::CmdCtx;
use super::sector_sel::parse_rel_xy;

// NatFlag bit for "capital was sacked" — not yet in NatFlags enum, use raw bit.
const NF_SACKED: u32 = 0x0040;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();

    // Determine target nation and coordinates
    // Formats:
    //   capital X,Y          (player)
    //   capital N X,Y        (deity)
    let (target_cnum, coord_str): (u8, &str) = if ctx.is_deity && parts.len() >= 2 {
        // Try parsing parts[0] as a nation number
        if let Ok(n) = parts[0].parse::<u8>() {
            if parts.len() < 2 {
                return "10 Usage: capital N X,Y\n".to_string();
            }
            (n, parts[1])
        } else {
            // Not a number — treat as "X,Y" for own nation
            (ctx.cnum, parts[0])
        }
    } else {
        match parts.first() {
            Some(s) => (ctx.cnum, s),
            None => return "10 Usage: capital X,Y\n".to_string(),
        }
    };

    let (rx, ry) = match parse_rel_xy(coord_str) {
        Some(xy) => xy,
        None => return format!("10 Bad coordinates '{}'\n", coord_str),
    };
    let abs_x = ctx.x_abs(rx);
    let abs_y = ctx.y_abs(ry);

    // Load target nation
    let mut nat = match nations::get_by_cnum(ctx.db, target_cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return format!("10 Country {} not found\n", target_cnum),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // Load the target sector
    let sect = match sectors::get_at(ctx.db, abs_x, abs_y).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist\n", ctx.format_xy(abs_x, abs_y)),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // Ownership check
    if sect.own != target_cnum && !ctx.is_deity {
        return format!("10 {} is not yours\n", ctx.format_xy(abs_x, abs_y));
    }

    // Type check: only urban (c) or mountain (^) can be capitals (non-deity)
    if !ctx.is_deity
        && sect.sector_type != SectorType::Capital
        && sect.sector_type != SectorType::Mountain
    {
        return format!(
            "10 {} is not a city or mountain sector\n",
            ctx.format_xy(abs_x, abs_y)
        );
    }

    let old_xcap = nat.xcap;
    let old_ycap = nat.ycap;

    // If origin matches old cap, update it too
    if nat.xorg == old_xcap && nat.yorg == old_ycap {
        nat.xorg = abs_x;
        nat.yorg = abs_y;
    }

    nat.xcap = abs_x;
    nat.ycap = abs_y;

    // Clear NF_SACKED if set
    if nat.flags.bits() & NF_SACKED != 0 {
        nat.flags = empire_types::nation::NatFlags::from_bits_truncate(
            nat.flags.bits() & !NF_SACKED
        );
    }

    if let Err(e) = nations::put(ctx.db, &nat).await {
        return format!("10 Database error saving nation: {e}\n");
    }

    let xy = ctx.format_xy(abs_x, abs_y);
    format!(
        "1 Country {}: capital moved to {}\n0 capital\n",
        target_cnum, xy
    )
}

pub async fn run_newcap(args: &str, ctx: &CmdCtx<'_>) -> String {
    if !ctx.is_deity {
        return "10 Permission denied: deity only\n".to_string();
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return "10 Usage: newcap <nation> <X,Y>\n".to_string();
    }

    let target_cnum: u8 = match parts[0].parse() {
        Ok(n) => n,
        Err(_) => return format!("10 Invalid nation number '{}'\n", parts[0]),
    };

    let (rx, ry) = match parse_rel_xy(parts[1]) {
        Some(xy) => xy,
        None => return format!("10 Bad coordinates '{}'\n", parts[1]),
    };
    let abs_x: Coord = ctx.x_abs(rx);
    let abs_y: Coord = ctx.y_abs(ry);

    // Load target nation
    let mut nat = match nations::get_by_cnum(ctx.db, target_cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return format!("10 Country {} not found\n", target_cnum),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // Load or create the sector
    let mut sect = match sectors::get_at(ctx.db, abs_x, abs_y).await {
        Ok(Some(s)) => s,
        Ok(None) => return format!("10 Sector {} doesn't exist in the world\n", ctx.format_xy(abs_x, abs_y)),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // If blank (unowned) wilderness, convert to Capital and give starting population
    if sect.own == 0 && sect.sector_type == SectorType::Wilderness {
        sect.sector_type = SectorType::Capital;
        sect.new_type = SectorType::Capital;
        sect.effic = 60;
        let mut inv = Inventory::zero();
        inv.set(Item::Civil, ctx.config.game.newcap_start_civ);
        inv.set(Item::Food,  ctx.config.game.newcap_start_food);
        sect.items = inv;
    }

    sect.own = target_cnum;

    if let Err(e) = sectors::put(ctx.db, &sect).await {
        return format!("10 Database error saving sector: {e}\n");
    }

    nat.xcap = abs_x;
    nat.ycap = abs_y;
    nat.xorg = abs_x;
    nat.yorg = abs_y;

    if let Err(e) = nations::put(ctx.db, &nat).await {
        return format!("10 Database error saving nation: {e}\n");
    }

    let xy = ctx.format_xy(abs_x, abs_y);
    format!(
        "1 Country {}: new capital set at {}\n0 newcap\n",
        target_cnum, xy
    )
}

