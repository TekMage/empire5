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
// Ported from: src/lib/global/misc.c (techfact())

// Shared tech-scaling helper. Mirrors 4.4.1's `techfact(tech, base)`, already
// duplicated inline in a couple of places (e.g. radar_cmd.rs); promoted here
// so the new gunnery/torpedo code (which needs it three times) has one home.

/// Scale `base` by tech level. tech=0 gives base/4; higher tech
/// asymptotically approaches (but never reaches) `base` itself.
pub fn techfact(base: f64, tech: f64) -> f64 {
    base * (50.0 + tech) / (200.0 + tech)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn techfact_at_zero_tech() {
        assert_eq!(techfact(10.0, 0.0), 2.5);
    }

    #[test]
    fn techfact_at_high_tech() {
        assert_eq!(techfact(10.0, 100.0), 5.0);
    }

    #[test]
    fn techfact_asymptotes_toward_base() {
        let high = techfact(10.0, 1_000_000.0);
        assert!(high > 9.9 && high < 10.0);
    }
}
