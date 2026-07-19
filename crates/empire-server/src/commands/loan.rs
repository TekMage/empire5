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
// Ported from: src/lib/commands/loan.c

// "loan" command — peer-to-peer lending.
//
// Sub-commands:
//   loan list                        — show all loans involving this player
//   loan offer NATION AMOUNT RATE DAYS — offer a loan to another nation
//   loan accept LOT#                 — accept a loan offered to you
//   loan repay  LOT# AMOUNT          — repay some or all of a loan

use empire_db::{loans, nations, telegrams};
use empire_types::loan::{Loan, LoanStatus};
use super::ctx::CmdCtx;

pub async fn run(args: &str, ctx: &CmdCtx<'_>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let sub = parts.first().copied().unwrap_or("list");

    match sub {
        "list" | "l"   => cmd_list(ctx).await,
        "offer" | "o"  => cmd_offer(&parts[1..], ctx).await,
        "accept" | "a" => cmd_accept(&parts[1..], ctx).await,
        "repay" | "r"  => cmd_repay(&parts[1..], ctx).await,
        _ => format!("10 Unknown loan sub-command '{sub}'. Use: list, offer, accept, repay\n"),
    }
}

// ── list ─────────────────────────────────────────────────────────────────────

async fn cmd_list(ctx: &CmdCtx<'_>) -> String {
    let all = match loans::get_for_nation(ctx.db, ctx.cnum).await {
        Ok(v) => v,
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if all.is_empty() {
        return "1 You have no loans.\n0 loan\n".to_string();
    }

    let mut out = String::new();
    out.push_str("1  #  Lender    Borrower  Amount      Paid        Rate    Due         Status\n");
    out.push_str("1  -  --------  --------  ----------  ----------  ------  ----------  ---------\n");

    for loan in &all {
        let loaner_name = nation_name(ctx, loan.loaner).await;
        let loanee_name = nation_name(ctx, loan.loanee).await;
        let status_str = match loan.status {
            LoanStatus::Offered   => "Offered",
            LoanStatus::Active    => "Active",
            LoanStatus::Paid      => "Paid",
            LoanStatus::Defaulted => "Defaulted",
        };
        let due_str = format_timestamp(loan.due);
        out.push_str(&format!(
            "1 {:2}  {:<8}  {:<8}  {:10.2}  {:10.2}  {:5.1}%  {}  {}\n",
            loan.uid,
            &loaner_name[..loaner_name.len().min(8)],
            &loanee_name[..loanee_name.len().min(8)],
            loan.amount,
            loan.paid,
            loan.interest_rate * 100.0,
            due_str,
            status_str,
        ));
    }

    out.push_str("0 loan\n");
    out
}

// ── offer ────────────────────────────────────────────────────────────────────

async fn cmd_offer(parts: &[&str], ctx: &CmdCtx<'_>) -> String {
    // loan offer NATION AMOUNT RATE DAYS
    if parts.len() < 4 {
        return "10 Usage: loan offer NATION AMOUNT RATE DAYS\n".to_string();
    }

    // Resolve borrower nation
    let loanee_cnum = match resolve_nation(parts[0], ctx).await {
        Ok(c) => c,
        Err(e) => return format!("10 {e}\n"),
    };

    if loanee_cnum == ctx.cnum {
        return "10 You cannot offer a loan to yourself.\n".to_string();
    }

    let amount: f64 = match parts[1].parse() {
        Ok(v) if v > 0.0 => v,
        _ => return format!("10 Invalid amount '{}'\n", parts[1]),
    };

    let rate_pct: f64 = match parts[2].parse() {
        Ok(v) if v >= 0.0 => v,
        _ => return format!("10 Invalid interest rate '{}'\n", parts[2]),
    };

    let days: i64 = match parts[3].parse() {
        Ok(v) if v > 0 => v,
        _ => return format!("10 Invalid duration '{}' — must be positive integer\n", parts[3]),
    };

    // Ensure lender has the money
    let lender = match nations::get_by_cnum(ctx.db, ctx.cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: your nation not found\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if (lender.money as f64) < amount {
        return format!(
            "10 You don't have ${amount:.2} to offer. You have ${}\n",
            lender.money
        );
    }

    // Ensure borrower exists
    if nations::get_by_cnum(ctx.db, loanee_cnum).await
        .map(|r| r.is_none())
        .unwrap_or(true)
    {
        return format!("10 Nation #{loanee_cnum} not found\n");
    }

    let now = chrono::Utc::now().timestamp();
    let uid = match loans::next_uid(ctx.db).await {
        Ok(u) => u,
        Err(e) => return format!("10 Failed to allocate loan ID: {e}\n"),
    };

    let loan = Loan {
        uid,
        loaner: ctx.cnum,
        loanee: loanee_cnum,
        amount,
        paid: 0.0,
        interest_rate: rate_pct / 100.0,
        status: LoanStatus::Offered,
        created: now,
        due: now + days * 86400,
    };

    if let Err(e) = loans::put(ctx.db, &loan).await {
        return format!("10 Failed to save loan offer: {e}\n");
    }

    let _ = telegrams::send(
        ctx.db, loanee_cnum, ctx.cnum, telegrams::TEL_NORM,
        &format!("Country #{} has offered you a loan (#{uid})\n", ctx.cnum),
    ).await;

    let borrower_name = nation_name(ctx, loanee_cnum).await;
    format!(
        "1 Loan #{uid} offered to {borrower_name}: ${amount:.2} at {rate_pct:.1}% for {days} days\n\
         0 loan\n"
    )
}

// ── accept ───────────────────────────────────────────────────────────────────

async fn cmd_accept(parts: &[&str], ctx: &CmdCtx<'_>) -> String {
    // loan accept LOT#
    let uid: i32 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return "10 Usage: loan accept LOT#\n".to_string(),
    };

    let mut loan = match loans::get_by_uid(ctx.db, uid).await {
        Ok(Some(l)) => l,
        Ok(None) => return format!("10 Loan #{uid} not found\n"),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if loan.loanee != ctx.cnum {
        return format!("10 Loan #{uid} was not offered to you\n");
    }

    if loan.status != LoanStatus::Offered {
        return format!("10 Loan #{uid} is not in Offered status\n");
    }

    // Transfer funds from loaner to loanee
    let mut loaner_nat = match nations::get_by_cnum(ctx.db, loan.loaner).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: lender nation not found\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    let mut loanee_nat = match nations::get_by_cnum(ctx.db, ctx.cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: your nation not found\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if (loaner_nat.money as f64) < loan.amount {
        return format!(
            "10 The lender no longer has ${:.2} — loan offer cancelled\n",
            loan.amount
        );
    }

    loaner_nat.money -= loan.amount as i32;
    loanee_nat.money += loan.amount as i32;
    loan.status = LoanStatus::Active;

    let r1 = nations::put(ctx.db, &loaner_nat).await;
    let r2 = nations::put(ctx.db, &loanee_nat).await;
    let r3 = loans::put(ctx.db, &loan).await;

    for r in [r1, r2, r3] {
        if let Err(e) = r {
            return format!("10 Database error during accept: {e}\n");
        }
    }

    let lender_name = nation_name(ctx, loan.loaner).await;
    format!(
        "1 Accepted loan #{uid} from {lender_name}: ${:.2} at {:.1}%\n\
         1 Funds transferred to your treasury.\n\
         0 loan\n",
        loan.amount,
        loan.interest_rate * 100.0
    )
}

// ── repay ────────────────────────────────────────────────────────────────────

async fn cmd_repay(parts: &[&str], ctx: &CmdCtx<'_>) -> String {
    // loan repay LOT# AMOUNT
    if parts.len() < 2 {
        return "10 Usage: loan repay LOT# AMOUNT\n".to_string();
    }

    let uid: i32 = match parts[0].parse() {
        Ok(v) => v,
        Err(_) => return format!("10 Invalid lot number '{}'\n", parts[0]),
    };

    let payment: f64 = match parts[1].parse() {
        Ok(v) if v > 0.0 => v,
        _ => return format!("10 Invalid amount '{}'\n", parts[1]),
    };

    let mut loan = match loans::get_by_uid(ctx.db, uid).await {
        Ok(Some(l)) => l,
        Ok(None) => return format!("10 Loan #{uid} not found\n"),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if loan.loanee != ctx.cnum {
        return format!("10 Loan #{uid} does not belong to you\n");
    }

    if loan.status != LoanStatus::Active {
        return format!("10 Loan #{uid} is not active (status: {:?})\n", loan.status);
    }

    // Check borrower has the money
    let mut loanee_nat = match nations::get_by_cnum(ctx.db, ctx.cnum).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: your nation not found\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    if (loanee_nat.money as f64) < payment {
        return format!(
            "10 You don't have ${payment:.2} to repay. You have ${}\n",
            loanee_nat.money
        );
    }

    let mut loaner_nat = match nations::get_by_cnum(ctx.db, loan.loaner).await {
        Ok(Some(n)) => n,
        Ok(None) => return "10 Internal error: lender nation not found\n".to_string(),
        Err(e) => return format!("10 Database error: {e}\n"),
    };

    // Compute how much is actually still owed
    let now = chrono::Utc::now().timestamp();
    let owed = loan.total_owed(now);
    let actual_payment = payment.min(owed);

    loanee_nat.money -= actual_payment as i32;
    loaner_nat.money += actual_payment as i32;
    loan.paid += actual_payment;

    // Check if fully paid
    if loan.total_owed(now) <= 0.0 {
        loan.status = LoanStatus::Paid;
    }

    let r1 = nations::put(ctx.db, &loanee_nat).await;
    let r2 = nations::put(ctx.db, &loaner_nat).await;
    let r3 = loans::put(ctx.db, &loan).await;

    for r in [r1, r2, r3] {
        if let Err(e) = r {
            return format!("10 Database error during repayment: {e}\n");
        }
    }

    let notice = if loan.status == LoanStatus::Paid {
        format!(
            "Country #{} paid off loan #{uid} with ${actual_payment:.0}\n",
            ctx.cnum
        )
    } else {
        format!(
            "Country #{} paid ${actual_payment:.0} on loan {uid}\n",
            ctx.cnum
        )
    };
    let _ = telegrams::send(
        ctx.db, loan.loaner, ctx.cnum, telegrams::TEL_NORM, &notice,
    ).await;

    let lender_name = nation_name(ctx, loan.loaner).await;
    let status_note = if loan.status == LoanStatus::Paid {
        "1 Loan fully repaid.\n".to_string()
    } else {
        let remaining = loan.total_owed(now);
        format!("1 Remaining balance: ${remaining:.2}\n")
    };

    format!(
        "1 Repaid ${actual_payment:.2} on loan #{uid} to {lender_name}\n\
         {status_note}\
         0 loan\n"
    )
}

// ── helpers ───────────────────────────────────────────────────────────────────

async fn nation_name(ctx: &CmdCtx<'_>, cnum: u8) -> String {
    match nations::get_by_cnum(ctx.db, cnum).await {
        Ok(Some(n)) => n.name,
        _ => format!("#{cnum}"),
    }
}

/// Resolve a nation identifier (number or name prefix) to a cnum.
async fn resolve_nation(arg: &str, ctx: &CmdCtx<'_>) -> Result<u8, String> {
    if let Ok(n) = arg.parse::<u8>() {
        return Ok(n);
    }
    let all = nations::get_all(ctx.db)
        .await
        .map_err(|e| format!("database error: {e}"))?;
    let arg_lc = arg.to_lowercase();
    all.into_iter()
        .find(|n| n.name.to_lowercase().starts_with(&arg_lc))
        .map(|n| n.cnum)
        .ok_or_else(|| format!("No such country: {arg}"))
}

/// Format a Unix timestamp as a human-readable date.
fn format_timestamp(ts: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let secs_since_epoch = if ts > 0 { ts as u64 } else { return "---".to_string() };
    // Minimal date formatting without chrono calendar parsing
    // Uses days since epoch to build a rough "YYYY-MM-DD" display
    let _ = UNIX_EPOCH + Duration::from_secs(secs_since_epoch);
    // Delegate to chrono for clean formatting
    let dt = chrono::DateTime::from_timestamp(ts, 0)
        .unwrap_or_else(chrono::Utc::now);
    dt.format("%Y-%m-%d").to_string()
}
