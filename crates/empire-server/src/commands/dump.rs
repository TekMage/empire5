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

// "dump" / "sdump" / "ldump" / "pdump" commands — dump game state in
// the classic Empire 4 format that ptkei's ParseDump parser expects.
//
// Format:
//   DUMP <TYPE> <timestamp>
//   <field-names>
//   <values...>
//   N <unit>s
//
// Coordinates are player-relative. Only the requesting player's own
// records are returned (matching Empire 4 behavior).

use super::ctx::CmdCtx;
use empire_db::sectors;
use empire_types::commodity::Item;

pub async fn run(subcmd: &str, ctx: &CmdCtx<'_>) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    match subcmd {
        "dump"  => dump_sectors(ts, ctx).await,
        "sdump" => empty_dump("SHIPS",      "ships",  "sdump", ts),
        "ldump" => empty_dump("LAND UNITS", "units",  "ldump", ts),
        "pdump" => empty_dump("PLANES",     "planes", "pdump", ts),
        _       => format!("10 Unknown dump subcommand\n"),
    }
}

async fn dump_sectors(ts: i64, ctx: &CmdCtx<'_>) -> String {
    let all = match sectors::get_all(ctx.db).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let mine: Vec<_> = all.into_iter()
        .filter(|s| s.own == ctx.cnum)
        .collect();

    let mut out = String::new();
    out.push_str(&format!("1 DUMP SECTOR {ts}\n"));
    out.push_str("1 x y des sdes eff mob * off min gold fert ocontent uran work avail terr civ mil uw food shell gun pet iron dust bar oil lcm hcm rad\n");

    for s in &mine {
        let des    = s.sector_type.mnemonic();
        let rx     = ctx.x_rel(s.x);
        let ry     = ctx.y_rel(s.y);
        let inv    = &s.items;
        out.push_str(&format!(
            "1 {rx} {ry} {des} _ {eff} {mob} . 0 {min} 0 {fert} {ocont} {uran} {work} 0 0 {civ} {mil} {uw} {food} {shell} {gun} {pet} {iron} {dust} {bar} {oil} {lcm} {hcm} {rad}\n",
            eff   = s.effic,
            mob   = s.mobil,
            min   = s.mines,
            fert  = s.fertil,
            ocont = s.oil,
            uran  = s.uran,
            work  = s.work,
            civ   = inv.get(Item::Civil),
            mil   = inv.get(Item::Milit),
            uw    = inv.get(Item::Uw),
            food  = inv.get(Item::Food),
            shell = inv.get(Item::Shell),
            gun   = inv.get(Item::Gun),
            pet   = inv.get(Item::Petrol),
            iron  = inv.get(Item::Iron),
            dust  = inv.get(Item::Dust),
            bar   = inv.get(Item::Bar),
            oil   = inv.get(Item::Oil),
            lcm   = inv.get(Item::Lcm),
            hcm   = inv.get(Item::Hcm),
            rad   = inv.get(Item::Rad),
        ));
    }

    out.push_str(&format!("1 {} sectors\n", mine.len()));
    out.push_str("0 dump\n");
    out
}

// Outputs an empty dump for ship/land/plane types — no records but valid
// header and footer so ParseDump terminates cleanly.
fn empty_dump(dump_type: &str, unit_name: &str, cmd_ok: &str, ts: i64) -> String {
    format!(
        "1 DUMP {dump_type} {ts}\n1 uid own x y type eff mob\n1 0 {unit_name}\n0 {cmd_ok}\n"
    )
}
