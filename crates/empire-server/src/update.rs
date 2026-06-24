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
// Ported from: src/server/update.c
// Known contributors to the original:
//    Dave Pare, 1994
//    Steve McClure, 1996
//    Ron Koenderink, 2005
//    Markus Armbruster, 2007-2020

// Update engine — fires on a configurable schedule.
// Replaces server/update.c and the empth_create(update, ...) cooperative thread.
//
// Phase 0: skeleton that acquires the exclusive write lock and logs a tick.
// Phase 3 will implement the full economic/military update cycle.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::info;

use empire_config::UpdateConfig;
use crate::journal::Journal;
use crate::state::GameState;

/// Run the update loop indefinitely.  Called as a Tokio task from main.
pub async fn run_update_loop(
    state: Arc<RwLock<GameState>>,
    cfg: UpdateConfig,
    journal: Arc<Journal>,
) {
    let interval_secs = cfg.update_interval_secs.max(60);
    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    info!(interval_secs, "Update engine started");

    loop {
        ticker.tick().await;

        // Acquire exclusive write lock — all player command tasks are blocked
        // from making game-state changes while this runs.
        // (Equivalent to empth_rwlock_wrlock(update_lock) in the C server.)
        let mut gs = state.write().await;
        gs.update_number += 1;
        let tick = gs.update_number;

        info!(tick, "Update tick starting");
        journal.update(tick);

        // Phase 3: run_economic_update(&mut gs).await;
        // Phase 3: run_military_update(&mut gs).await;
        // Phase 3: run_market_update(&mut gs).await;

        info!(tick, "Update tick complete");
        drop(gs); // Release write lock — player tasks can resume
    }
}
