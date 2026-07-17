#!/usr/bin/env python3
"""Hourly autopilot for country 1 on TekBot.

Runs standalone (stdlib only, no deps) against the raw Empire wire protocol
-- no C client, no TTY, so it works fine from cron. Scope is deliberately
narrow ("lightweight"): each tick, explore outward from every owned sector
that has mobility, then seed a little food into anything newly claimed so it
doesn't starve before the next real update. No ferrying, no designation
heuristics, no combat -- those stay manual/periodic-review territory.

Meant to be invoked hourly via cron on the TekBot host itself, talking to
127.0.0.1:6665 (the published container port). Logs every action taken to
a local file so a human (or a future Claude session) can review what
happened between check-ins.

Usage:
    python3 country1_bot.py [--host HOST] [--port PORT] [--country N]
                             [--user NAME] [--log PATH] [--dry-run]
"""

import argparse
import datetime
import socket
import sys

DIRS = "ujnbgy"


class ProtocolError(RuntimeError):
    pass


class EmpireClient:
    """Minimal client for the Empire5 line-oriented wire protocol.

    Wire format (crates/empire-server/src/protocol.rs): every line is
    "<code> <message>\\n". Code 1 = data, 0 = this command's own trailing
    line, 6 = PROMPT (ready for next command). We drain lines until a
    PROMPT line, treating everything else as this command's output.
    """

    def __init__(self, host, port, timeout=15):
        self.sock = socket.create_connection((host, port), timeout=timeout)
        self.sock.settimeout(timeout)
        self._buf = b""
        greeting = self._read_line()
        if not greeting.startswith("2 "):
            raise ProtocolError(f"unexpected greeting: {greeting!r}")

    def _read_line(self):
        while b"\n" not in self._buf:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise ProtocolError("connection closed by server")
            self._buf += chunk
        line, _, self._buf = self._buf.partition(b"\n")
        return line.decode("utf-8", errors="replace")

    def _send_raw(self, line):
        self.sock.sendall((line + "\n").encode("utf-8"))

    def _drain_to_prompt(self):
        lines = []
        while True:
            line = self._read_line()
            if line.startswith("6 "):
                return lines
            lines.append(line)

    def login(self, user, country, password=""):
        # Stepwise login rather than the single-line "play user coun pass":
        # the server .trim()s each incoming line before parsing, so an
        # empty trailing password token in "play u c " never survives to
        # be seen as a 4th argument -- "pass" with truly no argument is
        # the only reliable way to authenticate a blank-password nation.
        self._send_raw(f"user {user}")
        self._expect_cmdok()
        self._send_raw(f"coun {country}")
        self._expect_cmdok()
        self._send_raw(f"pass {password}".rstrip())
        self._expect_cmdok()
        self._send_raw("play")
        init = self._read_line()
        if not init.startswith("2 "):
            raise ProtocolError(f"login rejected: {init!r}")
        # MOTD lines then the first prompt.
        self._drain_to_prompt()

    def _expect_cmdok(self):
        line = self._read_line()
        if not line.startswith("0 "):
            raise ProtocolError(f"login step failed: {line!r}")

    def cmd(self, line):
        """Send one command, return its data lines with the "N " code
        prefix stripped (the trailing "0 <cmdname>" line included)."""
        self._send_raw(line)
        raw = self._drain_to_prompt()
        out = []
        for l in raw:
            _, _, text = l.partition(" ")
            out.append(text)
        return out

    def close(self):
        try:
            self._send_raw("quit")
        except OSError:
            pass
        finally:
            self.sock.close()


def parse_dump(lines):
    """Parse `dump`/`sdump` output (header line then space-separated rows,
    with the last column possibly a quoted, space-containing name) into a
    list of dicts keyed by the header's field names."""
    # First line is "DUMP SECTOR <ts>" / "DUMP SHIPS <ts>" -- skip it.
    # Second line is the field-name header.
    if len(lines) < 2:
        return []
    header = lines[1].split()
    rows = []
    for line in lines[2:]:
        if line.endswith(" sectors") or line.endswith(" ships") or \
           line.endswith(" sector") or line.endswith(" ship"):
            break  # trailing count line
        parts = line.split(maxsplit=len(header) - 1)
        if len(parts) < len(header):
            continue
        rows.append(dict(zip(header, parts)))
    return rows


def sector_key(row):
    return (int(row["x"]), int(row["y"]))


def log_line(log_path, msg):
    ts = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    line = f"[{ts}] {msg}"
    print(line)
    if log_path:
        with open(log_path, "a") as f:
            f.write(line + "\n")


def explore_wave(client, sectors, log_path, dry_run):
    """Try exploring in all 6 directions from every owned sector that has
    mobility and either civs or mil to spare. Cheap and safe: failed
    attempts (ocean, mountain, already-claimed) cost nothing."""
    claims = []
    for row in sectors:
        if int(row["mob"]) <= 0:
            continue
        civ = int(row["civ"])
        mil = int(row["mil"])
        if civ >= 10:
            item, amt = "civ", 5
        elif mil >= 10:
            item, amt = "mil", 5
        else:
            continue

        x, y = row["x"], row["y"]
        for d in DIRS:
            if dry_run:
                continue
            resp = client.cmd(f"explore {x},{y} {d} {item} {amt}")
            text = " ".join(resp)
            if "is now yours" in text:
                claims.append(text)
                log_line(log_path, f"explore {x},{y} {d}: {text.strip()}")
    return claims


def seed_new_claims(client, before, after, log_path, dry_run):
    """Any sector present in `after` but not `before` is brand new this
    tick (0% eff, 0 mobility). Give it a little food from the nearest
    owned harbor/warehouse with a healthy surplus, so it doesn't starve
    before the next real update's distribution pass reaches it."""
    before_keys = {sector_key(r) for r in before}
    new_rows = [r for r in after if sector_key(r) not in before_keys]
    if not new_rows:
        return

    hubs = [
        r for r in after
        if r["des"] in ("h", "w") and int(r["food"]) > 200
    ]

    for nr in new_rows:
        nx, ny = sector_key(nr)
        if not hubs:
            log_line(log_path, f"new claim {nx},{ny}: no food hub available to seed")
            continue
        # Cheap approximate distance -- doesn't need to match the game's
        # true hex/mobility cost, just needs to rank candidates sensibly.
        hub = min(hubs, key=lambda h: abs(int(h["x"]) - nx) + abs(int(h["y"]) - ny))
        hx, hy = hub["x"], hub["y"]
        if dry_run:
            log_line(log_path, f"[dry-run] would seed {nx},{ny} with food from {hx},{hy}")
            continue
        resp = client.cmd(f"move food {hx},{hy} 20 {nx},{ny}")
        text = " ".join(resp).strip()
        log_line(log_path, f"seed {nx},{ny} from {hx},{hy}: {text}")


def run_tick(host, port, user, country, password, log_path, dry_run):
    client = EmpireClient(host, port)
    try:
        client.login(user, country, password)
        before = parse_dump(client.cmd("dump"))
        log_line(log_path, f"tick start: {len(before)} owned sectors")

        claims = explore_wave(client, before, log_path, dry_run)

        after = parse_dump(client.cmd("dump")) if claims and not dry_run else before
        seed_new_claims(client, before, after, log_path, dry_run)

        log_line(log_path, f"tick done: {len(claims)} new sector(s) claimed, "
                            f"{len(after)} total owned")
    finally:
        client.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=6665)
    ap.add_argument("--country", default="1")
    ap.add_argument("--user", default="country1-bot")
    ap.add_argument("--password", default="")
    ap.add_argument("--log", default=None, help="Path to append-only log file")
    ap.add_argument("--dry-run", action="store_true",
                     help="Connect and dump state, but issue no commands")
    args = ap.parse_args()

    try:
        run_tick(args.host, args.port, args.user, args.country, args.password,
                  args.log, args.dry_run)
    except Exception as e:  # noqa: BLE001 -- cron job, must not hang or crash noisily
        log_line(args.log, f"ERROR: {type(e).__name__}: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
