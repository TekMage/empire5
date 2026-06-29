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
use super::sector_sel::SectSpec;

const ALL_ITEMS: [Item; 14] = [
    Item::Civil, Item::Milit, Item::Shell, Item::Gun, Item::Petrol,
    Item::Iron, Item::Dust, Item::Bar, Item::Food, Item::Oil,
    Item::Lcm, Item::Hcm, Item::Uw, Item::Rad,
];

fn parse_item(s: &str) -> Option<Item> {
    if s.is_empty() { return None; }
    if s.len() == 1 {
        return Item::from_mnemonic(s.chars().next()?);
    }
    let s_lc = s.to_lowercase();
    match s_lc.as_str() {
        "dust" | "gold dust"                    => return Some(Item::Dust),
        "bar"  | "bars" | "gold bars"           => return Some(Item::Bar),
        "lcm"  | "light"                        => return Some(Item::Lcm),
        "hcm"  | "heavy"                        => return Some(Item::Hcm),
        "uw"   | "undesirable" | "undesirables" => return Some(Item::Uw),
        _ => {}
    }
    for &item in &ALL_ITEMS {
        if item.name().starts_with(s_lc.as_str()) {
            return Some(item);
        }
    }
    None
}

/// Parse direction from either a char ('u','j','n','b','g','y','$','.') or
/// a numeric string optionally prefixed with '+' (ptkei format: +0..+6, +$).
fn parse_dir_str(s: &str) -> Option<u8> {
    let s = s.trim_start_matches('+');
    // Numeric form: 0=stop, 1=UR, 2=R, 3=DR, 4=DL, 5=L, 6=UL, 7=dist
    if let Ok(n) = s.parse::<u8>() {
        if n <= 7 { return Some(n); }
    }
    // Single-char form
    match s.chars().next()? {
        '.' => Some(0),
        'u' => Some(1),
        'j' => Some(2),
        'n' => Some(3),
        'b' => Some(4),
        'g' => Some(5),
        'y' => Some(6),
        '$' => Some(7),
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
    // Accepts two forms:
    //   4-arg: <item> <sect-spec> <threshold> <direction>
    //   3-arg: <item> <sect-spec> <direction>   (ptkei SetDel form; keeps current threshold)
    // Direction may be a char (u j n b g y $ .) or +N numeric (ptkei: +0..+6, +$).
    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.len() < 3 {
        return "10 Usage: deliver <item> <sect-spec> [threshold] <direction>\n".to_string();
    }

    let item = match parse_item(parts[0].trim()) {
        Some(i) => i,
        None => return format!("10 Unknown commodity: '{}'\n", parts[0]),
    };

    let area_spec = parts[1].trim();

    // Determine whether we have a threshold arg or just a direction.
    // If parts[2] starts with '+' or is a known direction char it's the direction.
    let (set_threshold, threshold_val, dir_str) = if parts.len() == 4 {
        let t: i16 = match parts[2].trim().parse() {
            Ok(v) => v,
            Err(_) => return format!("10 Invalid threshold '{}'\n", parts[2]),
        };
        (true, t, parts[3].trim())
    } else {
        // 3-arg: no threshold provided — only change direction
        (false, 0i16, parts[2].trim())
    };

    let dir: u8 = match parse_dir_str(dir_str) {
        Some(d) => d,
        None => return format!("10 Unknown direction '{}'; use: . u j n b g y $ or +0..+7\n", dir_str),
    };

    let filter = match SectSpec::parse(area_spec, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
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
        if !filter.matches(&s, ctx.world_x, ctx.world_y) {
            continue;
        }

        let xy = ctx.format_xy(s.x, s.y);
        let old_thresh = s.del[item_idx].threshold;
        let old_dir    = s.del[item_idx].path;

        if set_threshold { s.del[item_idx].threshold = threshold_val; }
        s.del[item_idx].path = dir;

        match sectors::put(ctx.db, &s).await {
            Ok(_) => {
                let new_thresh = if set_threshold { threshold_val } else { old_thresh };
                out.push_str(&format!(
                    "1 {} {} delivery: was thresh={} dir='{}', now thresh={} dir='{}'\n",
                    xy, item.name(),
                    old_thresh, dir_char(old_dir),
                    new_thresh, dir_char(dir)
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
