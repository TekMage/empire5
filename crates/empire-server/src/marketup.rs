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
//! `check_market` — expire stale bought lots and refund buyers after 24 h.
//! `check_trade`  — detect and mark loans that have passed their due date.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, info};

use empire_db::Db;
use empire_types::loan::LoanStatus;
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

        let gs = state.read().await;
        check_market(&gs.db).await;
        check_trade(&gs.db).await;
        drop(gs);
    }
}

/// Expire stale bought lots: refund the buyer and delete the lot.
///
/// A bought lot that is older than 24 hours is considered undeliverable
/// (the C server would have performed the delivery already via check_market
/// in buy.c; here we clean up any that slipped through or were manually
/// marked bought=true without delivery completing).
async fn check_market(db: &Db) {
    let lots = empire_db::trades::get_all(db).await.unwrap_or_default();
    let now = chrono::Utc::now().timestamp();

    for lot in lots.iter().filter(|l| l.bought && (now - l.created) > 86400) {
        // Refund buyer
        if let Ok(Some(mut buyer_nat)) =
            empire_db::nations::get_by_cnum(db, lot.buyer).await
        {
            let refund = lot.amount as f64 * lot.price;
            buyer_nat.money += refund as i32;
            let _ = empire_db::nations::put(db, &buyer_nat).await;
            debug!(
                "Expired trade lot #{}: refunded ${:.2} to buyer #{}",
                lot.uid, refund, lot.buyer
            );
        }
        let _ = empire_db::trades::delete(db, lot.uid).await;
    }
}

/// Detect loans that have passed their due date and mark them Defaulted.
async fn check_trade(db: &Db) {
    let loans = empire_db::loans::get_all(db).await.unwrap_or_default();
    let now = chrono::Utc::now().timestamp();

    for mut loan in loans
        .into_iter()
        .filter(|l| l.status == LoanStatus::Active && now > l.due)
    {
        loan.status = LoanStatus::Defaulted;
        if let Err(e) = empire_db::loans::put(db, &loan).await {
            debug!("Failed to mark loan #{} defaulted: {e}", loan.uid);
        } else {
            debug!("Loan #{} defaulted (due {})", loan.uid, loan.due);
        }
    }
}
