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
// DB accessors for the loans table.
// ref: struct lonstr in loan.h

use sqlx::FromRow;
use empire_types::loan::{Loan, LoanStatus};
use crate::{Db, DbResult};

// ── Raw DB row ────────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct LoanRow {
    uid:           i64,
    loaner:        i64,
    loanee:        i64,
    amount:        f64,
    paid:          f64,
    interest_rate: f64,
    status:        i64,
    created:       i64,
    due:           i64,
}

// ── Conversions ───────────────────────────────────────────────────────────────

fn row_to_loan(r: LoanRow) -> Option<Loan> {
    let status = LoanStatus::try_from_i32(r.status as i32)?;
    Some(Loan {
        uid:           r.uid as i32,
        loaner:        r.loaner as u8,
        loanee:        r.loanee as u8,
        amount:        r.amount,
        paid:          r.paid,
        interest_rate: r.interest_rate,
        status,
        created:       r.created,
        due:           r.due,
    })
}

// ── Reads ─────────────────────────────────────────────────────────────────────

/// Return every loan row in uid order.
pub async fn get_all(db: &Db) -> DbResult<Vec<Loan>> {
    let rows = sqlx::query_as::<_, LoanRow>(
        "SELECT uid,loaner,loanee,amount,paid,interest_rate,status,created,due \
         FROM loans ORDER BY uid",
    )
    .fetch_all(db.pool())
    .await?;

    Ok(rows.into_iter().filter_map(row_to_loan).collect())
}

/// Return the loan with the given uid, or `None` if absent.
pub async fn get_by_uid(db: &Db, uid: i32) -> DbResult<Option<Loan>> {
    let row = sqlx::query_as::<_, LoanRow>(
        "SELECT uid,loaner,loanee,amount,paid,interest_rate,status,created,due \
         FROM loans WHERE uid = ?",
    )
    .bind(uid)
    .fetch_optional(db.pool())
    .await?;

    Ok(row.and_then(row_to_loan))
}

/// Return all loans where `cnum` is either the lender or the borrower.
pub async fn get_for_nation(db: &Db, cnum: u8) -> DbResult<Vec<Loan>> {
    let rows = sqlx::query_as::<_, LoanRow>(
        "SELECT uid,loaner,loanee,amount,paid,interest_rate,status,created,due \
         FROM loans WHERE loaner = ? OR loanee = ? ORDER BY uid",
    )
    .bind(cnum as i64)
    .bind(cnum as i64)
    .fetch_all(db.pool())
    .await?;

    Ok(rows.into_iter().filter_map(row_to_loan).collect())
}

// ── Writes ────────────────────────────────────────────────────────────────────

/// Insert or replace a loan record.
pub async fn put(db: &Db, l: &Loan) -> DbResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO loans \
         (uid,loaner,loanee,amount,paid,interest_rate,status,created,due) \
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(l.uid)
    .bind(l.loaner as i64)
    .bind(l.loanee as i64)
    .bind(l.amount)
    .bind(l.paid)
    .bind(l.interest_rate)
    .bind(l.status as i32 as i64)
    .bind(l.created)
    .bind(l.due)
    .execute(db.pool())
    .await?;
    Ok(())
}

/// Return the next available uid (max existing + 1, minimum 1).
pub async fn next_uid(db: &Db) -> DbResult<i32> {
    let row: (Option<i64>,) =
        sqlx::query_as("SELECT MAX(uid) FROM loans")
            .fetch_one(db.pool())
            .await?;
    Ok(row.0.unwrap_or(0) as i32 + 1)
}
