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
// Ported from: src/lib/global/news.c, include/news.h

// News verb codes and their display properties.
// Mirrors the rpt[] table and N_* constants from 4.4.1.
//
// Each verb has:
//   page()      — which news section it appears in
//   good_will() — niceness score (negative = hostile act)
//   story()     — two alternate text strings; use %s for victim's country name

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NewsVerb {
    WonSect      = 1,   // N_WON_SECT   — infantry capture territory
    SctLose      = 2,   // N_SCT_LOSE   — infantry beaten back
    SentTel      = 4,   // N_SENT_TEL   — sends a telegram
    SctShell     = 10,  // N_SCT_SHELL  — gunners bombard a sector
    ShpShell     = 11,  // N_SHP_SHELL  — shells a ship
    TookUnocc    = 12,  // N_TOOK_UNOCC — takes unoccupied land
    ShpBomb      = 17,  // N_SHP_BOMB   — planes bomb a ship
    SctBomb      = 16,  // N_SCT_BOMB   — planes bomb a sector
    SubBomb      = 53,  // N_SUB_BOMB   — planes bomb a submarine
    Launch       = 40,  // N_LAUNCH     — launches a satellite into orbit
    ShipTorp     = 52,  // N_SHIP_TORP  — ship torpedoed
    DeclAlly     = 26,  // N_DECL_ALLY  — announces alliance
    DeclWar      = 28,  // N_DECL_WAR   — declares war
    DisAlly      = 29,  // N_DIS_ALLY   — disavows alliance
    DisWar       = 30,  // N_DIS_WAR    — ends war
    UpFriendly   = 61,  // N_UP_FRIENDLY
    DownFriendly = 62,  // N_DOWN_FRIENDLY
    UpNeutral    = 63,  // N_UP_NEUTRAL
    DownNeutral  = 64,  // N_DOWN_NEUTRAL
    UpHostile    = 65,  // N_UP_HOSTILE
    DownHostile  = 66,  // N_DOWN_HOSTILE
    AwonSect     = 73,  // N_AWON_SECT  — navy secures beachhead
    AloseSct     = 76,  // N_ALOSE_SCT  — sailors repelled
}

/// News page sections (matches N_* page constants in 4.4.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewsPage {
    Foreign,   // N_FOR  = 1
    FrontLine, // N_FRONT = 2
    Sea,       // N_SEA  = 3
    Sky,       // N_SKY  = 4
    Telecom,   // N_TELE = 11
}

impl NewsPage {
    pub fn heading(self) -> &'static str {
        match self {
            NewsPage::Foreign   => "Foreign Affairs",
            NewsPage::FrontLine => "The Front Line",
            NewsPage::Sea       => "The High Seas",
            NewsPage::Sky       => "Sky Watch",
            NewsPage::Telecom   => "Telecommunications",
        }
    }

    pub fn order(self) -> u8 {
        match self {
            NewsPage::Foreign   => 1,
            NewsPage::FrontLine => 2,
            NewsPage::Sea       => 3,
            NewsPage::Sky       => 4,
            NewsPage::Telecom   => 11,
        }
    }
}

impl NewsVerb {
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1  => Self::WonSect,
            2  => Self::SctLose,
            4  => Self::SentTel,
            10 => Self::SctShell,
            11 => Self::ShpShell,
            12 => Self::TookUnocc,
            16 => Self::SctBomb,
            17 => Self::ShpBomb,
            53 => Self::SubBomb,
            40 => Self::Launch,
            52 => Self::ShipTorp,
            26 => Self::DeclAlly,
            28 => Self::DeclWar,
            29 => Self::DisAlly,
            30 => Self::DisWar,
            61 => Self::UpFriendly,
            62 => Self::DownFriendly,
            63 => Self::UpNeutral,
            64 => Self::DownNeutral,
            65 => Self::UpHostile,
            66 => Self::DownHostile,
            73 => Self::AwonSect,
            76 => Self::AloseSct,
            _  => return None,
        })
    }

    pub fn page(self) -> NewsPage {
        match self {
            Self::WonSect | Self::SctLose | Self::TookUnocc | Self::SctShell => NewsPage::FrontLine,
            Self::AwonSect | Self::AloseSct | Self::ShpShell | Self::ShipTorp
                | Self::ShpBomb | Self::SubBomb                => NewsPage::Sea,
            Self::SentTel                                   => NewsPage::Telecom,
            Self::SctBomb | Self::Launch                    => NewsPage::Sky,
            _ /* declare / relations */                     => NewsPage::Foreign,
        }
    }

    /// Niceness score: negative = hostile act; positive = friendly act.
    pub fn good_will(self) -> i32 {
        match self {
            Self::WonSect      => -4,
            Self::SctLose      => -4,
            Self::SentTel      =>  1,
            Self::TookUnocc    =>  0,
            Self::SctBomb      => -2,
            Self::ShpBomb      => -2,
            Self::SubBomb      =>  0,
            Self::Launch       =>  0,
            Self::SctShell     => -2,
            Self::ShpShell     => -2,
            Self::ShipTorp     =>  0,
            Self::DeclAlly     =>  5,
            Self::DeclWar      => -5,
            Self::DisAlly      =>  0,
            Self::DisWar       =>  5,
            Self::UpFriendly   =>  3,
            Self::DownFriendly =>  0,
            Self::UpNeutral    =>  2,
            Self::DownNeutral  =>  0,
            Self::UpHostile    =>  3,
            Self::DownHostile  =>  3,
            Self::AwonSect     => -4,
            Self::AloseSct     =>  4,
        }
    }

    /// Story template — `%s` is replaced by victim's country name.
    /// Returns (story_a, story_b); display randomly picks one.
    pub fn stories(self) -> (&'static str, &'static str) {
        match self {
            Self::WonSect      => ("infantry capture %s territory",
                                   "shock troops overrun one of %s's sectors"),
            Self::SctLose      => ("infantry beaten back by %s troops",
                                   "shock troops annihilated in failed attack on %s"),
            Self::SentTel      => ("sends a telegram to %s",
                                   "telexes %s"),
            Self::TookUnocc    => ("takes over unoccupied land",
                                   "attacks unowned land for some reason"),
            Self::SctBomb      => ("planes dive-bomb one of %s's sectors",
                                   "bombers wreak havoc on %s"),
            Self::ShpBomb      => ("dive-bombs a ship flying the flag of %s",
                                   "air force bombs %s ships"),
            Self::SubBomb      => ("planes bomb a skulking %s submarine",
                                   "planes drop depth-charges on a %s sub"),
            Self::Launch       => ("launches a satellite into orbit",
                                   "continues its conquest of space with a successful launch"),
            Self::SctShell     => ("gunners bombard %s territory",
                                   "artillery fires on %s sectors"),
            Self::ShpShell     => ("shells a ship owned by %s",
                                   "fires on %s ships"),
            Self::ShipTorp     => ("ships torpedoed by %s torpedo-boats",
                                   "ships sunk by marauding %s torpedo-boats"),
            Self::DeclAlly     => ("announces an alliance with %s",
                                   "/ %s alliance declared"),
            Self::DeclWar      => ("declares TOTAL WAR on %s",
                                   "gets serious with %s and declares WAR"),
            Self::DisAlly      => ("diplomats disavow former alliance with %s",
                                   "is no longer allied with %s"),
            Self::DisWar       => ("is no longer at war with %s",
                                   "Foreign Ministry declares \"No more war with %s\""),
            Self::UpFriendly   => ("announces friendly trade relations with %s",
                                   "upgrades %s's trade status to triple-A"),
            Self::DownFriendly => ("downgrades relations with %s to friendly",
                                   "cools relations with %s to friendly"),
            Self::UpNeutral    => ("upgrades relations with %s to neutral",
                                   "Foreign Ministry declares \"%s is A-OK.\""),
            Self::DownNeutral  => ("downgrades relations with %s to neutral",
                                   "gives the cold shoulder to %s and declares neutral relations"),
            Self::UpHostile    => ("upgrades relations with %s to hostile",
                                   "forgives %s of past war crimes but remains hostile"),
            Self::DownHostile  => ("downgrades relations with %s to hostile",
                                   "is suspicious that %s has hostile intentions"),
            Self::AwonSect     => ("navy secures a beachhead on %s territory",
                                   "sailors take a coastal sector from %s"),
            Self::AloseSct     => ("sailors repelled by %s coast-guard",
                                   "naval forces massacred in failed assault of %s"),
        }
    }

    /// Whether this verb captures a sector from the victim (for Bottom Line).
    pub fn captures_sector(self) -> bool {
        matches!(self, Self::WonSect | Self::AwonSect)
    }
}
