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
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// See files COPYING and CREDITS in the root of the source tree for
// related information and legal notices.
//
// Ported from: src/util/empdump.c
// Known contributors to the original:
//    Markus Armbruster, 2008-2014

// empdump: export game state from the Empire 5 SQLite database in
// xdump text format.  Each table is written to stdout.
//
// Usage: empdump [-e CONFIG] [--db PATH] [TABLE...]
//
// TABLE may be: nation, sector, ship, plane, land, nuke, trade
// Omitting TABLE dumps all tables.

use std::path::PathBuf;
use anyhow::{Context, Result};
use clap::Parser;
use chrono::Utc;

use empire_config::load_or_default;
use empire_db::{Db, nations, sectors, ships, planes, land_units, nukes};

#[derive(Parser, Debug)]
#[command(
    name = "empdump",
    about = "Export Empire game state as xdump text"
)]
struct Args {
    /// Server config file (default: config/empire.toml)
    #[arg(short = 'e', long = "config", default_value = "config/empire.toml")]
    config: PathBuf,

    /// Path to empire.db (overrides config data_dir)
    #[arg(long = "db")]
    db_path: Option<PathBuf>,

    /// Tables to dump: nation sector ship plane land nuke trade
    /// (default: all)
    tables: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let cfg = load_or_default(&args.config);

    let db_path = if let Some(p) = args.db_path {
        p
    } else {
        cfg.server.data_dir.join("empire.db")
    };

    let db = Db::open(&db_path).await
        .with_context(|| format!("opening database {}", db_path.display()))?;

    let ts = Utc::now().timestamp();

    let all = args.tables.is_empty();
    let want = |name: &str| all || args.tables.iter().any(|t| t.eq_ignore_ascii_case(name));

    if want("nation") {
        let rows = nations::get_all(&db).await.context("loading nations")?;
        print!("{}", empire_db::xdump::dump_nations(&rows, ts));
    }
    if want("sector") {
        let rows = sectors::get_all(&db).await.context("loading sectors")?;
        print!("{}", empire_db::xdump::dump_sectors(&rows, ts));
    }
    if want("ship") {
        let rows = ships::get_all(&db).await.context("loading ships")?;
        print!("{}", empire_db::xdump::dump_ships(&rows, ts));
    }
    if want("plane") {
        let rows = planes::get_all(&db).await.context("loading planes")?;
        print!("{}", empire_db::xdump::dump_planes(&rows, ts));
    }
    if want("land") {
        let rows = land_units::get_all(&db).await.context("loading land units")?;
        print!("{}", empire_db::xdump::dump_land_units(&rows, ts));
    }
    if want("nuke") {
        let rows = nukes::get_all(&db).await.context("loading nukes")?;
        print!("{}", empire_db::xdump::dump_nukes(&rows, ts));
    }

    Ok(())
}
