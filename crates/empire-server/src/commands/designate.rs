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
// Ported from: src/lib/commands/desi.c

// "designate" command — redesignate sectors to a new type.
// Usage: designate <sector-spec> <type-char>
//   e.g. designate 0,0 c   (redesignate 0,0 to urban)
// When a sector is redesignated, its efficiency drops to 0 (must rebuild).
// Cannot designate water, mountain, or deity-only types.
// Harbor requires coastal sector.

use empire_db::sectors;
use empire_types::sector::SectorType;
use empire_types::sector_chr::SectorChr;
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return "10 Usage: designate <sector-spec> <type-char>\n".to_string();
    }
    let area_spec = parts[0];
    let type_str  = parts[1].trim();

    // Parse new sector type from mnemonic character
    let new_type = match parse_sector_type(type_str) {
        Some(t) => t,
        None => return format!("10 Unknown sector type: '{type_str}'\n"),
    };

    // Validate: cannot redesignate to water, mountain, or deity-only
    {
        let dchr = SectorChr::for_type(new_type);
        if new_type == SectorType::Sea || new_type == SectorType::Mountain {
            return format!("10 Bad sector type '{}'\n", type_str);
        }
        if dchr.is_deity && !ctx.is_deity {
            return format!("10 Bad sector type '{}'\n", type_str);
        }
    }

    let sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut out = String::new();
    let mut changed = 0u32;

    for mut s in sectors {
        // Only player's own sectors (or deity)
        if s.own != ctx.cnum && !ctx.is_deity { continue; }
        if s.own == 0 { continue; }
        if !matches_area(&s, area_spec, ctx) { continue; }

        let xy = ctx.format_xy(s.x, s.y);

        // Harbor requires coastal
        if new_type == SectorType::Harbor && !s.coastal {
            out.push_str(&format!("1 {xy} is not a coastal sector\n"));
            continue;
        }

        if s.sector_type == new_type {
            continue; // already correct type
        }

        // Check: cannot redesignate water or mountain sectors
        if s.sector_type == SectorType::Sea || s.sector_type == SectorType::Mountain {
            out.push_str(&format!("1 {xy}: cannot redesignate water/mountain\n"));
            continue;
        }

        s.effic = 0;
        s.sector_type = new_type;
        s.new_type = new_type;

        match sectors::put(ctx.db, &s).await {
            Ok(_) => {
                out.push_str(&format!(
                    "1 {xy} redesignated to '{}'\n",
                    new_type.mnemonic()
                ));
                changed += 1;
            }
            Err(e) => {
                out.push_str(&format!("1 {xy}: database error: {e}\n"));
            }
        }
    }

    if changed == 0 && out.is_empty() {
        out.push_str("1 No sectors redesignated.\n");
    }
    out.push_str("0 designate\n");
    out
}

fn matches_area(s: &empire_types::sector::Sector, spec: &str, ctx: &CmdCtx) -> bool {
    if spec.is_empty() || spec == "*" {
        return true;
    }
    if let Some((rx, ry)) = parse_rel_xy(spec) {
        return s.x == ctx.x_abs(rx) && s.y == ctx.y_abs(ry);
    }
    if let Some(pos) = spec.find(':') {
        let (coord_part, dist_part) = spec.split_at(pos);
        let dist_part = &dist_part[1..];
        if let (Some((rx, ry)), Ok(dist)) = (parse_rel_xy(coord_part), dist_part.trim().parse::<i32>()) {
            let ax = ctx.x_abs(rx);
            let ay = ctx.y_abs(ry);
            let d = crate::subs::geo::map_dist(s.x, s.y, ax, ay, ctx.world_x, ctx.world_y);
            return d <= dist;
        }
    }
    true
}

fn parse_rel_xy(s: &str) -> Option<(i16, i16)> {
    let (xs, ys) = s.split_once(',')?;
    Some((xs.trim().parse().ok()?, ys.trim().parse().ok()?))
}

fn parse_sector_type(s: &str) -> Option<SectorType> {
    let ch = s.chars().next()?;
    Some(match ch {
        '.' => SectorType::Sea,
        '-' => SectorType::Land,
        '^' => SectorType::Mountain,
        'g' => SectorType::Agri,
        'u' => SectorType::Uranium,
        'p' => SectorType::Plain,
        'P' => SectorType::Park,
        'c' => SectorType::Urban,
        'r' => SectorType::Research,
        'w' => SectorType::Wasteland,
        'd' => SectorType::Defense,
        'b' => SectorType::Bank,
        'e' => SectorType::Engineer,
        'a' => SectorType::Airfield,
        'h' => SectorType::Highway,
        'j' => SectorType::Radar,
        'n' => SectorType::Naval,
        'm' => SectorType::Missile,
        '*' => SectorType::Harbor,
        'f' => SectorType::Fort,
        't' => SectorType::Tech,
        's' => SectorType::Bravery,
        'l' => SectorType::LightIndus,
        'k' => SectorType::HeavyIndus,
        'G' => SectorType::Gold,
        'o' => SectorType::Oil,
        _   => return None,
    })
}
