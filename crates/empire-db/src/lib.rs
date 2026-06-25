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
// Ported from: include/file.h, src/lib/common/file.c
// Known contributors to the original:
//    Dave Pare, 1989
//    Steve McClure, 2000
//    Markus Armbruster, 2005-2014

// Database layer for Empire 5.
// Wraps sqlx SQLite with typed accessors for each game-object table.

use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;
use tracing::info;

pub mod nations;
pub mod sectors;
pub mod ships;
pub mod planes;
pub mod land_units;
pub mod nukes;
pub mod relations;
pub mod scan;
pub mod xdump;
pub mod xundump;
pub mod trades;
pub mod loans;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("record not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type DbResult<T> = Result<T, DbError>;

/// Shared database handle passed to all subsystems.
#[derive(Clone, Debug)]
pub struct Db {
    pub(crate) pool: SqlitePool,
}

impl Db {
    /// Open (or create) the SQLite database at `path`, run migrations.
    pub async fn open(path: &Path) -> DbResult<Self> {
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let opts = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Full)
            .foreign_keys(true);

        let pool = SqlitePool::connect_with(opts).await?;

        info!("Running database migrations");
        Self::migrate(&pool).await?;

        Ok(Db { pool })
    }

    /// Run embedded SQL migrations in version order.
    async fn migrate(pool: &SqlitePool) -> DbResult<()> {
        // 001: always idempotent (CREATE TABLE IF NOT EXISTS)
        sqlx::raw_sql(include_str!("migrations/001_initial.sql"))
            .execute(pool).await?;

        // Subsequent migrations guarded by schema_version
        let version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 1) FROM schema_version")
                .fetch_one(pool).await?;

        if version < 2 {
            sqlx::raw_sql(include_str!("migrations/002_passwords.sql"))
                .execute(pool).await?;
        }

        let version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 1) FROM schema_version")
                .fetch_one(pool).await?;

        if version < 3 {
            sqlx::raw_sql(include_str!("migrations/003_che_fields.sql"))
                .execute(pool).await?;
        }

        let version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 1) FROM schema_version")
                .fetch_one(pool).await?;

        if version < 4 {
            sqlx::raw_sql(include_str!("migrations/004_thresholds_and_relations.sql"))
                .execute(pool).await?;
        }

        let version: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version), 1) FROM schema_version")
                .fetch_one(pool).await?;

        if version < 5 {
            sqlx::raw_sql(include_str!("migrations/005_trade_loans.sql"))
                .execute(pool).await?;
        }

        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
pub(crate) async fn test_db() -> Db {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql(include_str!("migrations/001_initial.sql"))
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("migrations/002_passwords.sql"))
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("migrations/003_che_fields.sql"))
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("migrations/004_thresholds_and_relations.sql"))
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("migrations/005_trade_loans.sql"))
        .execute(&pool).await.unwrap();
    Db { pool }
}
