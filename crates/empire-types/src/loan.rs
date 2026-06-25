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
// Ported from: include/loan.h (struct lonstr)

/// Lifecycle state of a loan.
///
/// Mirrors the `LS_*` constants in loan.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LoanStatus {
    /// Offered to the borrower but not yet accepted (LS_PROPOSED = 1).
    Offered   = 0,
    /// Accepted and funds transferred; loan is running (LS_SIGNED = 2).
    Active    = 1,
    /// Fully repaid; archived.
    Paid      = 2,
    /// Past the due date without full repayment.
    Defaulted = 3,
}

impl LoanStatus {
    /// Convert a raw SQLite integer to a `LoanStatus`.
    pub fn try_from_i32(v: i32) -> Option<LoanStatus> {
        match v {
            0 => Some(LoanStatus::Offered),
            1 => Some(LoanStatus::Active),
            2 => Some(LoanStatus::Paid),
            3 => Some(LoanStatus::Defaulted),
            _ => None,
        }
    }
}

/// A peer-to-peer loan record.
///
/// Maps to the `loans` SQLite table.
/// Corresponds to C's `struct lonstr` in loan.h.
#[derive(Debug, Clone)]
pub struct Loan {
    /// Unique loan ID (auto-assigned, starts at 1).
    pub uid: i32,
    /// Lender's country number (l_loner in C).
    pub loaner: u8,
    /// Borrower's country number (l_lonee in C).
    pub loanee: u8,
    /// Principal amount in dollars (l_amtdue initial value).
    pub amount: f64,
    /// Amount repaid so far in dollars (l_amtpaid in C).
    pub paid: f64,
    /// Annual interest rate as a fraction (0.05 = 5%).
    pub interest_rate: f64,
    /// Current lifecycle state.
    pub status: LoanStatus,
    /// Unix timestamp when the loan was created/offered.
    pub created: i64,
    /// Unix timestamp of the repayment due date (l_duedate in C).
    pub due: i64,
}

impl Loan {
    /// Compute the total amount owed including accrued interest.
    ///
    /// Uses simple interest: `principal * (1 + rate * elapsed_years)`.
    /// Elapsed time is measured from `created` to `at_time`.
    pub fn total_owed(&self, at_time: i64) -> f64 {
        if self.status != LoanStatus::Active {
            return (self.amount - self.paid).max(0.0);
        }
        let elapsed_secs = (at_time - self.created).max(0) as f64;
        let elapsed_years = elapsed_secs / (365.25 * 86400.0);
        let total = self.amount * (1.0 + self.interest_rate * elapsed_years);
        (total - self.paid).max(0.0)
    }
}
