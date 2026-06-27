// Empire - A multi-player, client/server Internet based war game.
// Copyright (C) 1986-2021, Dave Pare, Jeff Bailey, Thomas Ruschak,
//               Ken Stevens, Steve McClure, Markus Armbruster
// Copyright (C) 2024-2026, Dave Nye
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/lib/commands/real.c

// "realm" command — show or set a realm bounding box.
//
// Usage: realm
//        realm <n>
//        realm <n> <xl:xh,yl:yh>
//
// Realms are named rectangular regions (0..49) used as sector selectors (#0–#49).
// Coordinates are always displayed and entered in the player's relative frame.
// The DB stores absolute coordinates.

use empire_db::nations;
use empire_types::nation::Realm;
use empire_types::coords::Coord;
use super::ctx::CmdCtx;

const MAXNOR: u16 = 50;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();

    let realms = match nations::get_realms(ctx.db, ctx.cnum).await {
        Ok(v) => v,
        Err(e) => return format!("10 DB error: {e}\n"),
    };

    // Return existing realm or a default that displays as 0:0,0:0 (the capital).
    // Using absolute (0,0) would wrap to a nonsense relative coord; use the
    // player's origin so unset realms display sensibly.
    let cap_x = ctx.x_abs(0);
    let cap_y = ctx.y_abs(0);
    let find_realm = |n: u16| -> Realm {
        realms.iter().find(|r| r.realm == n).cloned().unwrap_or(Realm {
            uid: ctx.cnum as i32 * MAXNOR as i32 + n as i32,
            cnum: ctx.cnum,
            realm: n,
            xl: cap_x, xh: cap_x, yl: cap_y, yh: cap_y,
        })
    };

    let mut out = String::new();

    if parts.is_empty() {
        for n in 0..MAXNOR {
            out.push_str(&fmt_realm(ctx, &find_realm(n)));
        }
    } else {
        let num_str = parts[0].trim_start_matches('#');
        let n: u16 = match num_str.parse::<u16>() {
            Ok(v) if v < MAXNOR => v,
            _ => return format!("10 Realm number must be in the range 0:{}\n", MAXNOR - 1),
        };

        if parts.len() == 1 {
            out.push_str(&fmt_realm(ctx, &find_realm(n)));
        } else {
            match parse_area(parts[1], ctx) {
                Some((xl, xh, yl, yh)) => {
                    let mut r = find_realm(n);
                    r.xl = xl; r.xh = xh; r.yl = yl; r.yh = yh;
                    if let Err(e) = nations::put_realm(ctx.db, &r).await {
                        return format!("10 DB error: {e}\n");
                    }
                    out.push_str(&fmt_realm(ctx, &r));
                }
                None => return format!("10 Bad area spec: {}\n", parts[1]),
            }
        }
    }

    out.push_str("0 realm\n");
    out
}

fn fmt_realm(ctx: &CmdCtx, r: &Realm) -> String {
    let xl = ctx.x_rel(r.xl);
    let xh = ctx.x_rel(r.xh);
    let yl = ctx.y_rel(r.yl);
    let yh = ctx.y_rel(r.yh);
    format!("1 Realm #{} is {}:{},{}:{}\n", r.realm, xl, xh, yl, yh)
}

// Parse "xl:xh,yl:yh" (relative coords) → absolute coords.
fn parse_area(s: &str, ctx: &CmdCtx) -> Option<(Coord, Coord, Coord, Coord)> {
    let (x_part, y_part) = s.split_once(',')?;
    let (xl_s, xh_s) = x_part.split_once(':')?;
    let (yl_s, yh_s) = y_part.split_once(':')?;
    let xl: Coord = xl_s.trim().parse().ok()?;
    let xh: Coord = xh_s.trim().parse().ok()?;
    let yl: Coord = yl_s.trim().parse().ok()?;
    let yh: Coord = yh_s.trim().parse().ok()?;
    Some((ctx.x_abs(xl), ctx.x_abs(xh), ctx.y_abs(yl), ctx.y_abs(yh)))
}
