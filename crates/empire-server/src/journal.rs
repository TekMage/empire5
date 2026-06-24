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
// Ported from: src/lib/subs/journal.c
// Known contributors to the original:
//    Markus Armbruster, 2004-2012
//    Ron Koenderink, 2008

// Journal — append-only event log written to data/journal.
//
// Format (one line per event, matching the C server):
//
//   TIME THREAD EVENT DATA
//
// Events:
//   startup
//   shutdown
//   login CNUM HOSTADDR USER
//   logout CNUM
//   command NAME
//   input INPUT
//   update ETU

use std::fs::{File, OpenOptions};
use std::io::{Write, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::warn;

pub struct Journal {
    inner: Mutex<JournalInner>,
    path: PathBuf,
}

struct JournalInner {
    file: BufWriter<File>,
}

impl Journal {
    /// Open (or create) the journal file in append mode.
    pub fn open(data_dir: &Path) -> std::io::Result<Self> {
        let path = data_dir.join("journal");
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Journal {
            inner: Mutex::new(JournalInner { file: BufWriter::new(file) }),
            path,
        })
    }

    pub fn startup(&self) {
        self.write_entry("Main", "startup", "");
    }

    pub fn shutdown(&self) {
        self.write_entry("Main", "shutdown", "");
        if let Ok(mut g) = self.inner.lock() {
            let _ = g.file.flush();
        }
    }

    pub fn login(&self, thread: &str, cnum: u8, host: &str, user: &str) {
        self.write_entry(thread, "login", &format!("{cnum} {host} {user}"));
    }

    pub fn logout(&self, thread: &str, cnum: u8) {
        self.write_entry(thread, "logout", &cnum.to_string());
    }

    pub fn command(&self, thread: &str, name: &str) {
        self.write_entry(thread, "command", name);
    }

    pub fn input(&self, thread: &str, line: &str) {
        self.write_entry(thread, "input", line);
    }

    pub fn update(&self, etu: u64) {
        self.write_entry("Update", "update", &etu.to_string());
    }

    fn write_entry(&self, thread: &str, event: &str, data: &str) {
        let now = timestamp_str();
        let line = if data.is_empty() {
            format!("{now} {thread:<10} {event}\n")
        } else {
            format!("{now} {thread:<10} {event} {data}\n")
        };
        match self.inner.lock() {
            Ok(mut g) => {
                if let Err(e) = g.file.write_all(line.as_bytes()) {
                    warn!(path = %self.path.display(), error = %e, "journal write failed");
                }
                // Flush after each entry so the log is always up to date
                let _ = g.file.flush();
            }
            Err(e) => warn!(error = %e, "journal mutex poisoned"),
        }
    }
}

/// Format current time as "Mon Jan  1 00:00:00 1970" (matches C's ctime format).
fn timestamp_str() -> String {
    use std::time::Duration;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    // chrono is available; use it for formatting
    use chrono::{DateTime, Utc, TimeZone};
    let dt: DateTime<Utc> = Utc.timestamp_opt(secs as i64, 0).single()
        .unwrap_or_else(|| Utc::now());
    dt.format("%a %b %e %T %Y").to_string()
}
