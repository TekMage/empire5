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

// Shared sector-spec parsing used by threshold, deliver, distribute, and other
// commands that accept a sector specifier argument.
//
// SPEC SYNTAX
//   *               — all owned sectors
//   X,Y             — exact sector (relative coords)
//   X,Y:R           — within radius R of X,Y (relative coords)
//   #N              — realm N (looks up bounding box from DB)
//
// CONDITION SUFFIX  (appended after a '?', no space)
//   ?des=X          — only sectors with designation letter X
//                     e.g. ?des=m  → iron mines only
//                          ?des=a  → agribusiness only
//
// Combined examples:
//   #1?des=m        — realm #1 iron mines
//   *?des=h         — all harbors
//   0,0:5?des=f     — food sectors within 5 of origin

use empire_db::nations;
use empire_types::sector::Sector;
use empire_types::coords::Coord;
use super::ctx::CmdCtx;
use crate::subs::geo;

/// A resolved sector selector.  Build with `SectSpec::parse`.
pub struct SectSpec {
    kind: SectSpecKind,
    /// Optional designation-letter filter from ?des=X suffix.
    des_filter: Option<char>,
}

enum SectSpecKind {
    All,
    XY(Coord, Coord),
    Radius { cx: Coord, cy: Coord, dist: i32 },
    RealmBox { xl: Coord, xh: Coord, yl: Coord, yh: Coord },
}

impl SectSpec {
    /// Parse and resolve a sector spec string.
    /// Realm specs (`#N`) require a DB lookup; pass `ctx` for that.
    pub async fn parse(raw: &str, ctx: &CmdCtx<'_>) -> Result<Self, String> {
        // Split on '?' to separate spec from condition
        let (spec_part, cond_part) = match raw.split_once('?') {
            Some((s, c)) => (s, Some(c)),
            None => (raw, None),
        };

        // Parse condition suffix
        let des_filter = parse_condition(cond_part);

        // Parse spec part
        let kind = if spec_part.is_empty() || spec_part == "*" {
            SectSpecKind::All

        } else if let Some(realm_str) = spec_part.strip_prefix('#') {
            // #N — realm bounding box
            let n: u16 = realm_str.trim().parse()
                .map_err(|_| format!("Bad realm number: #{realm_str}"))?;
            let realms = nations::get_realms(ctx.db, ctx.cnum).await
                .map_err(|e| format!("DB error: {e}"))?;
            let r = realms.into_iter().find(|r| r.realm == n)
                .ok_or_else(|| format!("Realm #{n} not set"))?;
            SectSpecKind::RealmBox { xl: r.xl, xh: r.xh, yl: r.yl, yh: r.yh }

        } else if let Some(pos) = spec_part.find(':') {
            // X,Y:R — radius
            let (coord_part, dist_str) = spec_part.split_at(pos);
            let dist: i32 = dist_str[1..].trim().parse()
                .map_err(|_| format!("Bad radius in spec: {spec_part}"))?;
            let (rx, ry) = parse_rel_xy(coord_part)
                .ok_or_else(|| format!("Bad coordinates: {coord_part}"))?;
            SectSpecKind::Radius {
                cx: ctx.x_abs(rx),
                cy: ctx.y_abs(ry),
                dist,
            }

        } else if let Some((rx, ry)) = parse_rel_xy(spec_part) {
            // X,Y — exact
            SectSpecKind::XY(ctx.x_abs(rx), ctx.y_abs(ry))

        } else {
            return Err(format!("Unknown sector spec: {spec_part}"));
        };

        Ok(SectSpec { kind, des_filter })
    }

    /// Return true if sector `s` matches this spec.
    pub fn matches(&self, s: &Sector, world_x: i32, world_y: i32) -> bool {
        let pos_ok = match &self.kind {
            SectSpecKind::All => true,
            SectSpecKind::XY(x, y) => s.x == *x && s.y == *y,
            SectSpecKind::Radius { cx, cy, dist } => {
                geo::map_dist(s.x, s.y, *cx, *cy, world_x, world_y) <= *dist
            }
            SectSpecKind::RealmBox { xl, xh, yl, yh } => {
                in_range_wrap(s.x, *xl, *xh, world_x as i16)
                    && in_range_wrap(s.y, *yl, *yh, world_y as i16)
            }
        };
        if !pos_ok { return false; }
        if let Some(des) = self.des_filter {
            if s.sector_type.mnemonic() != des { return false; }
        }
        true
    }
}

/// Check if `v` is in the range [lo..=hi] with wrap-around at `wrap`.
/// `pub(crate)`: also used by plane_cmd.rs/ship_cmd.rs/land_cmd.rs for
/// their own `?realm=N` filter (unit specs already use `#N` to mean
/// "uid N" -- see plane_spec_matches et al -- so realm filtering for
/// units is spelled `?realm=N` instead, to avoid `#N` meaning two
/// different things depending on which command it's typed into).
pub(crate) fn in_range_wrap(v: i16, lo: i16, hi: i16, wrap: i16) -> bool {
    if hi >= lo {
        v >= lo && v <= hi
    } else {
        // Wrapped range: lo..wrap + 0..hi
        v >= lo || v <= hi
    }
}

/// Split a unit spec into its base (uid/range/list/wildcard/letter) part
/// and a `?key=value&key2=value2` condition suffix, used by
/// plane_cmd.rs/ship_cmd.rs/land_cmd.rs for `?realm=N` and `?type=X`.
pub(crate) fn parse_unit_filters(spec: &str) -> (&str, Vec<(&str, &str)>) {
    match spec.split_once('?') {
        Some((base, cond)) => {
            let filters = cond.split('&')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.trim(), v.trim()))
                .collect();
            (base, filters)
        }
        None => (spec, Vec::new()),
    }
}

/// Look up a realm by number from a parsed filter list, if a `realm=N`
/// filter is present. Returns `Ok(None)` if there's no realm filter,
/// `Err` if `realm=N` was given but isn't a valid/set realm number.
pub(crate) async fn resolve_realm_filter(
    filters: &[(&str, &str)], ctx: &CmdCtx<'_>,
) -> Result<Option<empire_types::nation::Realm>, String> {
    let Some((_, v)) = filters.iter().find(|(k, _)| *k == "realm") else {
        return Ok(None);
    };
    let n: u16 = v.parse().map_err(|_| format!("Bad realm number: realm={v}"))?;
    let realms = empire_db::nations::get_realms(ctx.db, ctx.cnum).await
        .map_err(|e| format!("DB error: {e}"))?;
    realms.into_iter().find(|r| r.realm == n)
        .map(Some)
        .ok_or_else(|| format!("Realm #{n} not set"))
}

fn parse_condition(cond: Option<&str>) -> Option<char> {
    let cond = cond?;
    // Support: des=X  (designation letter filter)
    if let Some(rest) = cond.strip_prefix("des=") {
        return rest.trim().chars().next();
    }
    // Unknown condition — ignore silently (forward-compatible)
    None
}

/// Convenience synchronous helper for commands that don't need realm support.
/// Falls back to SectSpec::parse semantics but returns bool directly.
/// NOTE: does NOT support `#N` — use `SectSpec::parse` for full support.
pub fn matches_area(s: &Sector, spec: &str, ctx: &CmdCtx<'_>) -> bool {
    // Split condition
    let (spec_part, cond_part) = match spec.split_once('?') {
        Some((s, c)) => (s, Some(c)),
        None => (spec, None),
    };
    let des_filter = parse_condition(cond_part);

    let pos_ok = if spec_part.is_empty() || spec_part == "*" {
        true
    } else if let Some(pos) = spec_part.find(':') {
        let (coord_part, dist_str) = spec_part.split_at(pos);
        if let (Some((rx, ry)), Ok(dist)) =
            (parse_rel_xy(coord_part), dist_str[1..].trim().parse::<i32>())
        {
            let ax = ctx.x_abs(rx);
            let ay = ctx.y_abs(ry);
            geo::map_dist(s.x, s.y, ax, ay, ctx.world_x, ctx.world_y) <= dist
        } else {
            true
        }
    } else if let Some((rx, ry)) = parse_rel_xy(spec_part) {
        s.x == ctx.x_abs(rx) && s.y == ctx.y_abs(ry)
    } else {
        true
    };

    if !pos_ok { return false; }
    if let Some(des) = des_filter {
        if s.sector_type.mnemonic() != des { return false; }
    }
    true
}

/// Parse "X,Y" relative coordinate string.
pub fn parse_rel_xy(s: &str) -> Option<(i16, i16)> {
    let (xs, ys) = s.split_once(',')?;
    Some((xs.trim().parse().ok()?, ys.trim().parse().ok()?))
}
