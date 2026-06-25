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
// Ported from: src/lib/commands/deli.c

// "deliver" command — set commodity delivery threshold and direction per sector.
// Usage: deliver <item> <sect-spec> <threshold> <direction>
//
// Direction encoding:
//   '.' = DIR_STOP (0) — stop delivery
//   'u' = DIR_UR   (1) — upper right
//   'j' = DIR_R    (2) — right
//   'n' = DIR_DR   (3) — lower right
//   'b' = DIR_DL   (4) — lower left
//   'g' = DIR_L    (5) — left
//   'y' = DIR_UL   (6) — upper left
//   '$' = DIR_DIST (7) — send to distribution center

use empire_db::sectors;
use empire_types::commodity::Item;
use super::ctx::CmdCtx;
use super::sector_sel::matches_area;

const ALL_ITEMS: [Item; 14] = [
    Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
    Item::Iron, Item::Dust, Item::Bar, Item::Food, Item::Oil,
    Item::Lcm, Item::Hcm, Item::Uw, Item::Rad,
];

fn parse_item(s: &str) -> Option<Item> {
    if s.is_empty() { return None; }
    if s.len() == 1 {
        if let Some(i) = Item::from_mnemonic(s.chars().next()?) {
            return Some(i);
        }
    }
    let s_lc = s.to_lowercase();
    for &item in &ALL_ITEMS {
        if item.name().starts_with(s_lc.as_str()) {
            return Some(item);
        }
    }
    None
}

/// Return the direction code (0-7) from a direction char, or None if unknown.
fn parse_dir(c: char) -> Option<u8> {
    match c {
        '.' => Some(0), // DIR_STOP
        'u' => Some(1), // DIR_UR
        'j' => Some(2), // DIR_R
        'n' => Some(3), // DIR_DR
        'b' => Some(4), // DIR_DL
        'g' => Some(5), // DIR_L
        'y' => Some(6), // DIR_UL
        '$' => Some(7), // DIR_DIST
        _ => None,
    }
}

fn dir_char(d: u8) -> char {
    match d {
        0 => '.', 1 => 'u', 2 => 'j', 3 => 'n',
        4 => 'b', 5 => 'g', 6 => 'y', 7 => '$',
        _ => '?',
    }
}

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    // Expected: <item> <sect-spec> <threshold> <dir>
    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return "10 Usage: deliver <item> <sect-spec> <threshold> <direction>\n".to_string();
    }

    let item = match parse_item(parts[0].trim()) {
        Some(i) => i,
        None => return format!("10 Unknown commodity: '{}'\n", parts[0]),
    };

    let area_spec  = parts[1].trim();
    let threshold_str = parts[2].trim();
    let dir_str    = parts[3].trim();

    let raw_thresh: i16 = match threshold_str.parse() {
        Ok(v) => v,
        Err(_) => return format!("10 Invalid threshold '{}'\n", threshold_str),
    };
    // Round threshold down to multiple of 8
    let threshold: i16 = raw_thresh & !7;

    let dir_char_input = dir_str.chars().next().unwrap_or(' ');
    let dir: u8 = match parse_dir(dir_char_input) {
        Some(d) => d,
        None => return format!("10 Unknown direction '{}'; use: . u j n b g y $\n", dir_char_input),
    };

    let all_sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let item_idx = item as usize;
    let mut out = String::new();
    let mut count = 0u32;

    for mut s in all_sectors {
        if s.own != ctx.cnum && !ctx.is_deity {
            continue;
        }
        if s.own == 0 {
            continue;
        }
        if !matches_area(&s, area_spec, ctx) {
            continue;
        }

        let xy = ctx.format_xy(s.x, s.y);
        let old_thresh = s.del[item_idx].threshold;
        let old_dir    = s.del[item_idx].path;

        s.del[item_idx].threshold = threshold;
        s.del[item_idx].path      = dir;

        match sectors::put(ctx.db, &s).await {
            Ok(_) => {
                out.push_str(&format!(
                    "1 {} {} delivery: was thresh={} dir='{}', now thresh={} dir='{}'\n",
                    xy, item.name(),
                    old_thresh, dir_char(old_dir),
                    threshold, dir_char(dir)
                ));
                count += 1;
            }
            Err(e) => {
                out.push_str(&format!("1 {}: database error: {e}\n", xy));
            }
        }
    }

    if count == 0 && out.is_empty() {
        out.push_str("1 No sectors matched\n");
    }
    out.push_str("0 deliver\n");
    out
}
