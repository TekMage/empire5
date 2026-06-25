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
// Ported from: src/util/empsched.c
// Known contributors to the original:
//    Markus Armbruster, 2007-2010

//! empsched — print the Empire update schedule.
//!
//! Usage: empsched [-e CONFIG] [-n COUNT] [FILE]
//!
//! Reads the schedule file (from config or FILE argument) and prints the next
//! COUNT scheduled update times to stdout.

use std::path::PathBuf;
use anyhow::{Context, Result, bail};
use clap::Parser;
use chrono::{DateTime, Duration, Local};

use empire_config::{load_or_default, rdsched};

const DFLT_N: usize = 16;

#[derive(Parser, Debug)]
#[command(
    name = "empsched",
    about = "Print the Empire update schedule"
)]
struct Cli {
    /// Read server config from this file (default: empire.toml)
    #[arg(short = 'e', long = "config")]
    config_file: Option<PathBuf>,

    /// Print at most NUMBER upcoming updates (default: 16)
    #[arg(short = 'n', long = "count", default_value_t = DFLT_N)]
    count: usize,

    /// Schedule file to read instead of the one from config
    file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config to get schedule_file path (if no explicit FILE given)
    let default_config = PathBuf::from("empire.toml");
    let config_path = cli.config_file.as_deref().unwrap_or(&default_config);
    let config = load_or_default(config_path);

    // Resolve schedule file path
    let sched_path = if let Some(f) = cli.file {
        f
    } else {
        config.server.schedule_file.clone()
    };

    if sched_path.as_os_str().is_empty() {
        bail!("No schedule file configured (set [server] schedule_file in empire.toml or pass FILE argument)");
    }

    if !sched_path.exists() {
        bail!("Schedule file not found: {}", sched_path.display());
    }

    // Reference time: current time rounded up to the next minute (C convention)
    let now = Local::now();
    let anchor = round_up_minute(now);
    // Show updates that come at least 1 second after now
    let after = now - Duration::seconds(1);

    let times = rdsched::read_schedule(&sched_path, after, anchor, cli.count)
        .with_context(|| format!("reading schedule file {}", sched_path.display()))?;

    if times.is_empty() {
        eprintln!("No scheduled updates found in {}", sched_path.display());
        return Ok(());
    }

    for t in &times {
        // Match C's ctime() output format: "Fri Jan  5 14:00:00 2007\n"
        println!("{}", t.format("%a %b %e %H:%M:%S %Y"));
    }

    Ok(())
}

fn round_up_minute(dt: DateTime<Local>) -> DateTime<Local> {
    let ts = dt.timestamp();
    let rounded = (ts + 59) / 60 * 60;
    match chrono::DateTime::from_timestamp(rounded, 0) {
        Some(utc) => utc.with_timezone(&Local),
        None => dt,
    }
}
