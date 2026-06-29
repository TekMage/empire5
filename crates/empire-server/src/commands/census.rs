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
// Ported from: src/lib/commands/cens.c

// "census" command — sector-by-sector report for all owned sectors.
// Usage: census [sector-spec]  (default: *)

use empire_db::sectors;
use empire_types::commodity::Item;
use empire_types::sector::Sector;
use super::ctx::CmdCtx;
use super::sector_sel::SectSpec;

// Direction chars for del[].path & 0x7 display.
// Index 0='.' (stop), 1-6=compass, 7='$' (distribute to dist center).
const DIRSTR: [char; 7] = ['.', 'u', 'j', 'n', 'b', 'g', 'y'];
fn dir_char(path: u8) -> char {
    let d = (path & 0x7) as usize;
    if d == 7 { '$' } else { DIRSTR[d] }
}

fn thresh_char(t: i16) -> char {
    if t == 0 { '.' } else { char::from_digit(((t / 100) % 10) as u32, 10).unwrap_or('?') }
}

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let spec_str = args.trim();
    let filter = match SectSpec::parse(if spec_str.is_empty() { "*" } else { spec_str }, ctx).await {
        Ok(f) => f,
        Err(e) => return format!("10 {e}\n"),
    };

    let sectors = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 database error: {e}\n"),
    };

    let mut matching: Vec<&Sector> = sectors.iter()
        .filter(|s| {
            if s.own == 0 { return false; }
            if s.own != ctx.cnum && !ctx.is_deity { return false; }
            filter.matches(s, ctx.world_x, ctx.world_y)
        })
        .collect();

    matching.sort_by_key(|s| (s.y, s.x));

    if matching.is_empty() {
        let arg = if spec_str.is_empty() { "*" } else { spec_str };
        return format!("1 {arg}: No sector(s)\n0 census\n");
    }

    let mut out = String::new();

    // Header
    if ctx.is_deity {
        out.push_str("1 CENSUS                   del dst\n");
        out.push_str("1 own   sect        eff prd mob uf uf old  civ  mil   uw food work avail fall coa\n");
    } else {
        out.push_str("1 CENSUS                   del dst\n");
        out.push_str("1   sect        eff prd mob uf uf old  civ  mil   uw food work avail ter  fall coa\n");
    }

    for s in &matching {
        out.push_str(&format_census_row(s, ctx));
    }

    let n = matching.len();
    out.push_str(&format!("1 {n} sector{}\n", if n == 1 { "" } else { "s" }));
    out.push_str("0 census\n");
    out
}


fn format_census_row(s: &Sector, ctx: &CmdCtx) -> String {
    let xy = ctx.format_xy(s.x, s.y);
    let type_ch = s.sector_type.mnemonic();
    let newtype_ch = if s.new_type != s.sector_type {
        s.new_type.mnemonic()
    } else {
        ' '
    };
    let off_str = if s.off { "no " } else { "   " };

    let uw_dir  = dir_char(s.del[Item::Uw   as usize].path);
    let fod_dir = dir_char(s.del[Item::Food as usize].path);
    let uw_thr  = thresh_char(s.del[Item::Uw   as usize].threshold);
    let fod_thr = thresh_char(s.del[Item::Food as usize].threshold);

    let oldown_str = if s.old_own != s.own {
        format!("{:3}", s.old_own)
    } else {
        "   ".to_string()
    };

    let civ  = s.items.get(Item::Civil);
    let mil  = s.items.get(Item::Milit);
    let uw   = s.items.get(Item::Uw);
    let food = s.items.get(Item::Food);
    let fall = s.fallout;
    let coa  = if s.coastal { format!("{:4}", 1) } else { String::new() };
    let terr = if !ctx.is_deity && s.terr[0] != 0 {
        format!("{:4}", s.terr[0])
    } else {
        "    ".to_string()
    };

    if ctx.is_deity {
        format!(
            "1 {:3} {:9} {}{}{:4}%{}{:4} {}{} {}{} {}  {:5}{:5}{:5}{:5}{:4}%{:6}{:5}{}\n",
            s.own, xy, type_ch, newtype_ch, s.effic, off_str, s.mobil,
            uw_dir, fod_dir, uw_thr, fod_thr, oldown_str,
            civ, mil, uw, food, s.work, s.avail, fall, coa
        )
    } else {
        format!(
            "1 {:9} {}{}{:4}%{}{:4} {}{} {}{} {}  {:5}{:5}{:5}{:5}{:4}%{:6}{}{:5}{}\n",
            xy, type_ch, newtype_ch, s.effic, off_str, s.mobil,
            uw_dir, fod_dir, uw_thr, fod_thr, oldown_str,
            civ, mil, uw, food, s.work, s.avail, terr, fall, coa
        )
    }
}
