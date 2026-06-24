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
// Ported from: src/lib/subs/natsub.c, src/lib/subs/natarg.c
// Known contributors to the original:
//    Markus Armbruster, 2009-2013
//    Ron Koenderink, 2008-2009

// Nation display and validation utilities.

use empire_types::nation::Nation;
use empire_types::coords::NatId;

// ── Nation display ────────────────────────────────────────────────────────────

/// Format a nation as "CountryName (#N)".
/// Equivalent to C's `prnat(np)`.
pub fn format_nat(nat: &Nation) -> String {
    format!("{} (#{})", nat.name, nat.cnum)
}

/// Format a nation by ID, falling back to "#N" if not found.
/// Equivalent to C's `prnatid(cnum)`.
pub fn format_nat_id(cnum: NatId, nations: &[Nation]) -> String {
    match nations.iter().find(|n| n.cnum == cnum) {
        Some(nat) => format_nat(nat),
        None      => format!("#{cnum}"),
    }
}

// ── Nation name validation ────────────────────────────────────────────────────

/// Validate a proposed country name.
/// Returns `Ok(())` if the name is valid and unique (ignoring `cnum`'s own entry).
/// Returns `Err(message)` with a player-facing error.
///
/// Rules (from C's `check_nat_name`):
///   - Must be non-empty and not all whitespace.
///   - No control characters.
///   - Length ≤ 19 characters (C used fixed char[20]).
///   - Must not duplicate any other nation's name.
///
/// Equivalent to C's `check_nat_name(cname, cnum)`.
pub fn check_nat_name(
    name: &str,
    cnum: NatId,
    existing: &[Nation],
) -> Result<(), String> {
    if name.len() > 19 {
        return Err("Country name too long".to_string());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("No control characters allowed in country names!".to_string());
    }
    if name.trim().is_empty() {
        return Err("Country name can't be all blank".to_string());
    }
    for n in existing {
        if n.cnum != cnum && n.name == name {
            return Err(format!("Country #{} is already called `{name}`", n.cnum));
        }
    }
    Ok(())
}

/// Parse a nation argument (number or prefix match of country name).
/// Returns the matching nation, or an error message.
///
/// If `arg` is purely numeric, treat as country number.
/// Otherwise, find nations whose names start with `arg` (case-sensitive).
/// Equivalent to the logic behind C's `natargp()` / `cnumb()`.
pub fn parse_nat_arg<'a>(arg: &str, nations: &'a [Nation]) -> Result<&'a Nation, String> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return Err("No country specified".to_string());
    }

    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        let n: u8 = trimmed.parse()
            .map_err(|_| format!("Invalid country number: {trimmed}"))?;
        return nations.iter().find(|nat| nat.cnum == n)
            .ok_or_else(|| format!("Country '{trimmed}' doesn't exist."));
    }

    let matches: Vec<_> = nations.iter()
        .filter(|n| n.name.starts_with(trimmed))
        .collect();

    match matches.len() {
        0 => Err(format!("Country '{trimmed}' doesn't exist.")),
        1 => Ok(matches[0]),
        _ => Err(format!("Country '{trimmed}' is ambiguous")),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use empire_types::nation::{Nation, NatStatus, NatFlags};

    fn make_nation(cnum: u8, name: &str) -> Nation {
        Nation {
            uid: cnum as i32, cnum, status: NatStatus::Active,
            flags: NatFlags::empty(),
            name: name.to_string(), representative: String::new(),
            host_addr: String::new(), user_id: String::new(),
            xcap: 0, ycap: 0, xorg: 0, yorg: 0,
            money: 0, reserve: 0,
            tech: 0.0, research: 0.0, education: 0.0, happiness: 0.0,
            login_count: 0, tele_cnt: 0,
            passwd_hash: String::new(), last_login: 0, last_logout: 0,
        }
    }

    #[test]
    fn format_nat_output() {
        let n = make_nation(3, "Freedonia");
        assert_eq!(format_nat(&n), "Freedonia (#3)");
    }

    #[test]
    fn check_nat_name_ok() {
        let nations = vec![make_nation(1, "Existing")];
        assert!(check_nat_name("NewName", 2, &nations).is_ok());
    }

    #[test]
    fn check_nat_name_duplicate() {
        let nations = vec![make_nation(1, "Existing")];
        let err = check_nat_name("Existing", 2, &nations).unwrap_err();
        assert!(err.contains("already called"));
    }

    #[test]
    fn check_nat_name_same_cnum_ok() {
        // Renaming your own country to the same name is OK
        let nations = vec![make_nation(1, "Existing")];
        assert!(check_nat_name("Existing", 1, &nations).is_ok());
    }

    #[test]
    fn check_nat_name_blank() {
        let err = check_nat_name("   ", 1, &[]).unwrap_err();
        assert!(err.contains("blank"));
    }

    #[test]
    fn parse_nat_arg_by_number() {
        let nations = vec![make_nation(2, "Freedonia")];
        let n = parse_nat_arg("2", &nations).unwrap();
        assert_eq!(n.cnum, 2);
    }

    #[test]
    fn parse_nat_arg_by_prefix() {
        let nations = vec![make_nation(2, "Freedonia"), make_nation(3, "Graustark")];
        let n = parse_nat_arg("Free", &nations).unwrap();
        assert_eq!(n.cnum, 2);
    }

    #[test]
    fn parse_nat_arg_ambiguous() {
        let nations = vec![make_nation(2, "Freedonia"), make_nation(3, "Freedom")];
        assert!(parse_nat_arg("Free", &nations).is_err());
    }
}
