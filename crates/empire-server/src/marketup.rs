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
// Ported from: src/server/marketup.c
// Known contributors to the original:
//    Steve McClure, 1996
//    Markus Armbruster, 2007-2013

//! Market update task — runs `check_market` + `check_trade` every 5 minutes.
//!
//! Only active when `config.game.opt_market` is true (mirrors C's `opt_MARKET`).
//!
//! The actual market/trade logic (`check_market`, `check_trade`) lives with the
//! `buy`/`sell`/`trade` commands, which are Phase 6+ scope.  Until those are
//! implemented this task logs a no-op trace message each cycle so the
//! infrastructure is in place and the log makes the cadence visible.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, info};

use crate::state::GameState;

/// Spawn the market update loop if `opt_market` is enabled.
/// Should be called once from `main` before the accept loop.
pub async fn run_market_loop(state: Arc<RwLock<GameState>>, opt_market: bool) {
    if !opt_market {
        info!("Market system disabled (opt_market = false)");
        return;
    }

    info!("Market update task started (every 300 s)");

    // Mirror C: sleep first, then check.  Runs every 5 minutes.
    let mut ticker = time::interval(Duration::from_secs(300));
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    ticker.tick().await; // consume the first immediate tick

    loop {
        ticker.tick().await;

        // Take a shared read lock — market checks read game state but do not
        // modify it (commodity prices are aggregated from stored lot records).
        let _gs = state.read().await;

        check_market();
        check_trade();

        drop(_gs);
    }
}

/// Expire stale commodity lots and recompute market prices.
///
/// Stub — full implementation requires `buy`/`sell` command tables (Phase 6+).
fn check_market() {
    debug!("check_market: stub (market commands not yet implemented)");
}

/// Expire stale trade offers and settle completed trades.
///
/// Stub — full implementation requires `trade`/`loan` command tables (Phase 6+).
fn check_trade() {
    debug!("check_trade: stub (trade commands not yet implemented)");
}
