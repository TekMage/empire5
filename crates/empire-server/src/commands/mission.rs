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
// Ported from: src/lib/commands/miss.c, src/lib/subs/mission.c
// Known contributors to the original:
//    Steve McClure, 1996-2000
//    Markus Armbruster, 2003-2021

// "mission" / "miss" command — set standing orders for ships, planes, or land units.
//
// Usage: mission UNIT-TYPE UNIT-SPEC MISSION-TYPE [RADIUS]
//
// UNIT-TYPE: 's' = ship, 'p' = plane, 'l' = land unit
// UNIT-SPEC: "*" for all owned, single uid, or comma-separated uids.
// MISSION-TYPE: letter code:
//   o = off (no mission)
//   i = intercept
//   s = support (allied attack support)
//   e = escort
//   r = reserve (react to attack)
//   a = air defense
//   p = pindown (suppress enemy planes)
//   b = besiege (siege a sector)
// RADIUS: optional reaction radius in sectors (default: max range of unit).

use empire_db::{ships, planes, land_units};

use super::ctx::CmdCtx;

/// Mission type codes — match C's M_* constants in include/mission.h.
#[allow(dead_code)]
mod mission_code {
    pub const OFF:       i16 = 0;
    pub const INTERCEPT: i16 = 1;
    pub const SUPPORT:   i16 = 2;
    pub const ESCORT:    i16 = 3;
    pub const RESERVE:   i16 = 4;
    pub const AIR_DEF:   i16 = 5;
    pub const PINDOWN:   i16 = 6;
    pub const BESIEGE:   i16 = 7;
}

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 3 {
        return "10 Usage: mission UNIT-TYPE UNIT-SPEC MISSION-TYPE [RADIUS]\n".to_string();
    }

    let unit_type = parts[0].to_lowercase();
    let unit_spec = parts[1];
    let mission_char = parts[2].chars().next().unwrap_or('o');
    let radius: i16 = parts.get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(i16::MAX);

    let mission_code = match parse_mission(mission_char) {
        Some(m) => m,
        None => return format!(
            "10 Unknown mission type '{}'. Use o/i/s/e/r/a/p/b.\n",
            mission_char
        ),
    };

    let mission_name = mission_name(mission_code);

    let mut out = String::new();
    let mut count = 0u32;

    match unit_type.as_str() {
        "s" | "ship" => {
            let all = match ships::get_all(ctx.db).await {
                Ok(v) => v,
                Err(e) => return format!("10 DB error: {e}\n"),
            };
            for mut ship in all {
                if ship.own != ctx.cnum && !ctx.is_deity { continue; }
                if !uid_matches(unit_spec, ship.uid) { continue; }
                ship.mission = mission_code;
                ship.mission_radius = radius;
                if let Err(e) = ships::put(ctx.db, &ship).await {
                    out.push_str(&format!("1 Ship #{}: save error: {e}\n", ship.uid));
                } else {
                    count += 1;
                }
            }
        }
        "p" | "plane" => {
            let all = match planes::get_all(ctx.db).await {
                Ok(v) => v,
                Err(e) => return format!("10 DB error: {e}\n"),
            };
            for mut plane in all {
                if plane.own != ctx.cnum && !ctx.is_deity { continue; }
                if !uid_matches(unit_spec, plane.uid) { continue; }
                plane.mission = mission_code;
                plane.mission_radius = radius;
                if let Err(e) = planes::put(ctx.db, &plane).await {
                    out.push_str(&format!("1 Plane #{}: save error: {e}\n", plane.uid));
                } else {
                    count += 1;
                }
            }
        }
        "l" | "land" => {
            let all = match land_units::get_all(ctx.db).await {
                Ok(v) => v,
                Err(e) => return format!("10 DB error: {e}\n"),
            };
            for mut unit in all {
                if unit.own != ctx.cnum && !ctx.is_deity { continue; }
                if !uid_matches(unit_spec, unit.uid) { continue; }
                unit.mission = mission_code;
                unit.mission_radius = radius;
                if let Err(e) = land_units::put(ctx.db, &unit).await {
                    out.push_str(&format!("1 Land unit #{}: save error: {e}\n", unit.uid));
                } else {
                    count += 1;
                }
            }
        }
        other => {
            return format!(
                "10 Unknown unit type '{other}'. Use s(ship), p(lane), l(and).\n"
            );
        }
    }

    if count == 0 {
        out.push_str("1 No matching units found.\n");
    } else {
        let radius_str = if radius == i16::MAX {
            "max".to_string()
        } else {
            radius.to_string()
        };
        out.push_str(&format!(
            "1 Set mission '{mission_name}' for {count} unit(s) (radius: {radius_str})\n"
        ));
    }

    out.push_str("0 mission\n");
    out
}

fn parse_mission(c: char) -> Option<i16> {
    match c.to_ascii_lowercase() {
        'o' => Some(mission_code::OFF),
        'i' => Some(mission_code::INTERCEPT),
        's' => Some(mission_code::SUPPORT),
        'e' => Some(mission_code::ESCORT),
        'r' => Some(mission_code::RESERVE),
        'a' => Some(mission_code::AIR_DEF),
        'p' => Some(mission_code::PINDOWN),
        'b' => Some(mission_code::BESIEGE),
        _   => None,
    }
}

fn mission_name(code: i16) -> &'static str {
    match code {
        mission_code::OFF       => "off",
        mission_code::INTERCEPT => "intercept",
        mission_code::SUPPORT   => "support",
        mission_code::ESCORT    => "escort",
        mission_code::RESERVE   => "reserve",
        mission_code::AIR_DEF   => "air defense",
        mission_code::PINDOWN   => "pindown",
        mission_code::BESIEGE   => "besiege",
        _                       => "unknown",
    }
}

/// Return true if `uid` matches the unit spec string.
fn uid_matches(spec: &str, uid: i32) -> bool {
    if spec == "*" {
        return true;
    }
    for part in spec.split(',') {
        let part = part.trim().trim_start_matches('#');
        if let Ok(n) = part.parse::<i32>() {
            if n == uid {
                return true;
            }
        }
    }
    false
}
