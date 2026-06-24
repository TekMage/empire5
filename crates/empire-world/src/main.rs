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
// Ported from: src/util/fairland.c, src/util/files.c
// Known contributors to the original:
//    Ken Stevens, 1995
//    Steve McClure, 1998
//    Markus Armbruster, 2004-2020

//! empire-world — world generator for Empire 5.
//!
//! Ports two C utilities:
//!   - `fairland`  — creates the sector topology from scratch
//!   - `files`     — initialises a blank DB with the deity nation
//!
//! Usage: empire-world [OPTIONS] NC SC [NI [IS [SP [PM [DI [ID]]]]]]
//!
//!   NC = number of continents
//!   SC = continent size (sectors)
//!   NI = number of islands (default: NC)
//!   IS = average island size (default: SC/2)
//!   SP = spike % (0=round, 100=snake; default: 10)
//!   PM = mountain % (default: 0)
//!   DI = min distance between continents (default: 2)
//!   ID = min distance from islands to continents (default: 1)

mod fairland;

use std::path::{Path, PathBuf};
use anyhow::{Context, Result, bail};
use clap::Parser;
use rand::random;

use empire_config::Config;
use empire_db::{Db, sectors, nations};
use empire_types::nation::{Nation, NatFlags, NatStatus};

use fairland::Fairland;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "empire-world",
    about = "Create an Empire game world (port of fairland + files)"
)]
struct Cli {
    /// Read config from this file (default: ./empire.toml)
    #[arg(short = 'e', long = "config")]
    config_file: Option<PathBuf>,

    /// Path to the game SQLite database (overrides config data_dir)
    #[arg(long = "db")]
    db_path: Option<PathBuf>,

    /// Name of the newcap script to write (default: newcap_script)
    #[arg(short = 's', long = "script", default_value = "newcap_script")]
    script: PathBuf,

    /// Random seed (default: random)
    #[arg(short = 'R', long = "seed")]
    seed: Option<u64>,

    /// Allow islands to merge (removes per-island exclusive zones)
    #[arg(short = 'i', long = "merge-islands")]
    merge_islands: bool,

    /// Quiet — suppress progress output
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// NC SC [NI [IS [SP [PM [DI [ID]]]]]]
    #[arg(required = true, num_args = 2..=8)]
    args: Vec<i32>,
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── Load config to get world dims + data_dir ───────────────────────────
    let config = load_config(cli.config_file.as_deref())?;
    let world_x = config.game.world_x as usize;
    let world_y = config.game.world_y as usize;

    // ── Resolve DB path ────────────────────────────────────────────────────
    let db_path = if let Some(p) = cli.db_path {
        p
    } else {
        std::fs::create_dir_all(&config.server.data_dir)
            .with_context(|| format!("creating data dir {}", config.server.data_dir.display()))?;
        config.server.data_dir.join("empire.db")
    };

    // ── Parse positional args: NC SC [NI [IS [SP [PM [DI [ID]]]]]] ────────
    let args = &cli.args;
    let nc = args[0] as usize;
    let sc = args[1] as usize;
    let ni = args.get(2).copied().unwrap_or(nc as i32) as usize;
    let is = args.get(3).copied().unwrap_or((sc / 2).max(1) as i32) as usize;
    let sp = args.get(4).copied().unwrap_or(10);
    let pm = args.get(5).copied().unwrap_or(0);
    let di = args.get(6).copied().unwrap_or(2);
    let id = args.get(7).copied().unwrap_or(1);

    validate_args(nc, sc, ni, is, sp, pm, di, id, world_x, world_y)?;

    let seed = cli.seed.unwrap_or_else(|| random::<u64>());

    // ── Open DB ────────────────────────────────────────────────────────────
    let db = Db::open(&db_path).await
        .with_context(|| format!("opening database {}", db_path.display()))?;
    println!("Database: {}", db_path.display());

    // ── Ensure deity nation exists ──────────────────────────────────────────
    ensure_deity(&db).await?;

    // ── Print parameters ───────────────────────────────────────────────────
    if !cli.quiet {
        println!();
        println!("Creating a planet with:");
        println!("{nc} continents");
        println!("continent size: {sc}");
        println!("number of islands: {ni}");
        println!("average size of islands: {is}");
        println!("spike: {sp}%");
        println!(
            "{pm}% of land is mountain (each continent will have {} mountains)",
            (pm * sc as i32) / 100
        );
        println!("minimum distance between continents: {di}");
        println!("minimum distance from islands to continents: {id}");
        println!("World dimensions: {world_x}x{world_y}");
        println!();
        println!("        #*# ...fairland rips open a rift in the datumplane... #*#");
        println!();
        println!("seed is {seed}");
    }

    // ── Run fairland with retries ──────────────────────────────────────────
    let distinct_islands = !cli.merge_islands;
    let mut world = None;

    for attempt in 0..fairland::NUMTRIES {
        let seed_i = seed.wrapping_add(attempt as u64);
        let mut fl = Fairland::new(
            world_x, world_y,
            nc, sc, ni, is,
            sp, pm, di, id,
            distinct_islands,
            cli.quiet,
            seed_i,
        );

        if attempt > 0 && !cli.quiet {
            println!("\ntry #{} (out of {})...", attempt + 1, fairland::NUMTRIES);
        }

        if !cli.quiet { println!("placing capitals..."); }
        if !fl.drift() && !cli.quiet { println!("unstable drift"); }

        if !cli.quiet { print!("growing continents..."); }
        if !fl.grow_continents() { continue; }
        if !cli.quiet { println!(); }

        if !cli.quiet { print!("growing islands:"); }
        if !fl.grow_islands() { continue; }

        if !cli.quiet { println!("elevating land..."); }
        fl.create_elevations();

        world = Some(fl);
        break;
    }

    let fl = match world {
        Some(f) => f,
        None => bail!(
            "world not large enough for this much land — gave up after {} tries",
            fairland::NUMTRIES
        ),
    };

    if !cli.quiet { fl.print_map(); }

    // ── Write sectors to DB ────────────────────────────────────────────────
    if !cli.quiet { println!("\nwriting to database..."); }
    let sector_list = fl.build_sectors();
    sectors::put_many(&db, &sector_list).await
        .context("writing sectors to database")?;
    if !cli.quiet { println!("wrote {} sectors.", sector_list.len()); }

    // ── Write newcap_script ────────────────────────────────────────────────
    write_newcap_script(&cli.script, fl.capitals(), nc)?;
    if !cli.quiet {
        println!(
            "\nA script for adding all countries can be found in \"{}\".",
            cli.script.display()
        );
    }

    // ── Ensure visitor nation slot exists ──────────────────────────────────
    ensure_visitor(&db, nc).await?;

    println!("\nWorld generation complete.");
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_config(path: Option<&Path>) -> Result<Config> {
    let default_path = PathBuf::from("empire.toml");
    let p = path.unwrap_or(&default_path);
    if p.exists() {
        let text = std::fs::read_to_string(p)
            .with_context(|| format!("reading config {}", p.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing config {}", p.display()))
    } else {
        Ok(Config::default())
    }
}

fn validate_args(
    nc: usize, sc: usize, ni: usize, is: usize,
    sp: i32, pm: i32, di: i32, id: i32,
    world_x: usize, world_y: usize,
) -> Result<()> {
    if nc < 1       { bail!("number of continents must be > 0"); }
    if sc < 2       { bail!("size of continents must be > 1"); }
    if ni % nc != 0 { bail!("number of islands must be a multiple of the number of continents"); }
    if is < 1       { bail!("size of islands must be > 0"); }
    if !(0..=100).contains(&sp) { bail!("spike percentage must be between 0 and 100"); }
    if !(0..=100).contains(&pm) { bail!("mountain percentage must be between 0 and 100"); }
    let dist_max = {
        let dx = (world_x / 2) as i32;
        let dy = (world_y / 2) as i32;
        if dx > dy { (dx - dy) / 2 + dy } else { dy }
    };
    if di < 0 || di > dist_max { bail!("distance between continents ({di}) out of range 0..={dist_max}"); }
    if id < 0 || id > dist_max { bail!("distance islands to continents ({id}) out of range 0..={dist_max}"); }
    Ok(())
}

async fn ensure_deity(db: &Db) -> Result<()> {
    if nations::get_by_cnum(db, 0).await?.is_some() {
        return Ok(());
    }
    let deity = Nation {
        uid: 0, cnum: 0,
        status: NatStatus::Deity,
        flags: NatFlags::empty(),
        name: "POGO".to_string(),
        representative: "peter".to_string(),
        host_addr: String::new(),
        user_id: String::new(),
        xcap: 0, ycap: 0,
        xorg: 0, yorg: 0,
        money: 0, reserve: 0,
        tech: 0.0, research: 0.0, education: 0.0, happiness: 0.0,
        login_count: 0, tele_cnt: 0,
        passwd_hash: String::new(),
        last_login: 0, last_logout: 0,
    };
    nations::put(db, &deity).await.context("creating deity nation")?;
    println!("All praise to POGO!");
    Ok(())
}

async fn ensure_visitor(db: &Db, nc: usize) -> Result<()> {
    let vcnum = (nc + 1) as u8;
    if nations::get_by_cnum(db, vcnum).await?.is_some() {
        return Ok(());
    }
    let visitor = Nation {
        uid: vcnum as i32, cnum: vcnum,
        status: NatStatus::Visitor,
        flags: NatFlags::empty(),
        name: "visitor".to_string(),
        representative: String::new(),
        host_addr: String::new(),
        user_id: String::new(),
        xcap: 0, ycap: 0,
        xorg: 0, yorg: 0,
        money: 0, reserve: 0,
        tech: 0.0, research: 0.0, education: 0.0, happiness: 0.0,
        login_count: 0, tele_cnt: 0,
        passwd_hash: String::new(),
        last_login: 0, last_logout: 0,
    };
    nations::put(db, &visitor).await.context("creating visitor nation")?;
    Ok(())
}

fn write_newcap_script(path: &Path, caps: &[(usize, usize)], nc: usize) -> Result<()> {
    use std::fmt::Write as FmtWrite;
    let mut script = String::new();
    for (c, &(x, y)) in caps.iter().enumerate() {
        let n = c + 1;
        writeln!(script, "add {n} {n} {n} p")?;
        writeln!(script, "newcap {n} {x},{y}")?;
    }
    writeln!(script, "add {} visitor visitor v", nc + 1)?;
    std::fs::write(path, &script)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
