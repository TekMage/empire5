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
// Ported from: include/nsc.h, src/lib/common/nsc.c
// Known contributors to the original:
//    Dave Pare, 1989
//    Markus Armbruster, 2004-2020

// NSC (Nation Selector Condition) types and parser.
// ref: include/nsc.h, src/lib/subs/nsc.c
//
// Grammar (simplified):
//   spec   ::= type area cond*
//   type   ::= "sect" | "ship" | "plane" | "land" | "nuke" | "nat" | "realm"
//   area   ::= "*" | "#" digit+
//   cond   ::= "?" field op value
//   op     ::= ">" | "<" | ">=" | "<=" | "=" | "#" | "~"
//   value  ::= integer | float | string

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    Nation,
    Realm,
    Sector,
    Ship,
    Plane,
    LandUnit,
    Nuke,
}

impl ObjectType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "nat" | "nation" | "nations" => Some(Self::Nation),
            "realm" | "realms"           => Some(Self::Realm),
            "sect" | "sector" | "sectors" => Some(Self::Sector),
            "ship" | "ships"             => Some(Self::Ship),
            "plane" | "planes"           => Some(Self::Plane),
            "land" | "unit" | "land_units" => Some(Self::LandUnit),
            "nuke" | "nukes"             => Some(Self::Nuke),
            _ => None,
        }
    }

    pub fn xdump_name(&self) -> &'static str {
        match self {
            Self::Nation  => "nation",
            Self::Realm   => "realm",
            Self::Sector  => "sector",
            Self::Ship    => "ship",
            Self::Plane   => "plane",
            Self::LandUnit => "land",
            Self::Nuke    => "nuke",
        }
    }
}

/// Selects which records to operate on.
#[derive(Debug, Clone)]
pub enum SelectArea {
    All,
    Uid(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Gt, Lt, Ge, Le, Eq, Ne,
}

#[derive(Debug, Clone)]
pub enum CondValue {
    Int(i64),
    Float(f64),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub field: String,
    pub op: CompareOp,
    pub value: CondValue,
}

#[derive(Debug, Clone)]
pub struct ScanSpec {
    pub object_type: ObjectType,
    pub area: SelectArea,
    pub conditions: Vec<Condition>,
}

// ── Parser ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "selector parse error: {}", self.0)
    }
}
impl std::error::Error for ParseError {}

/// Parse a selector string, e.g. `"sector * ?eff>50"` or `"ship #3"`.
pub fn parse_scan_spec(input: &str) -> Result<ScanSpec, ParseError> {
    let mut tokens = input.split_whitespace();

    let type_tok = tokens.next().ok_or_else(|| ParseError("empty input".into()))?;
    let object_type = ObjectType::from_str(type_tok)
        .ok_or_else(|| ParseError(format!("unknown object type '{type_tok}'")))?;

    let area_tok = tokens.next().unwrap_or("*");
    let area = parse_area(area_tok)?;

    let mut conditions = Vec::new();
    for tok in tokens {
        if let Some(rest) = tok.strip_prefix('?') {
            conditions.push(parse_condition(rest)?);
        }
        // silently skip unrecognised tokens (future-proofing)
    }

    Ok(ScanSpec { object_type, area, conditions })
}

fn parse_area(s: &str) -> Result<SelectArea, ParseError> {
    match s {
        "*" | "" => Ok(SelectArea::All),
        _ if s.starts_with('#') => {
            let n: i32 = s[1..].parse()
                .map_err(|_| ParseError(format!("bad uid in area '{s}'")))?;
            Ok(SelectArea::Uid(n))
        }
        _ => Err(ParseError(format!("unrecognised area '{s}'"))),
    }
}

fn parse_condition(s: &str) -> Result<Condition, ParseError> {
    // try two-char ops first so ">=" isn't parsed as ">" + "="
    let (field, op, val_str) = if let Some(i) = find_op(s, ">=") {
        (&s[..i], CompareOp::Ge, &s[i+2..])
    } else if let Some(i) = find_op(s, "<=") {
        (&s[..i], CompareOp::Le, &s[i+2..])
    } else if let Some(i) = find_op(s, ">") {
        (&s[..i], CompareOp::Gt, &s[i+1..])
    } else if let Some(i) = find_op(s, "<") {
        (&s[..i], CompareOp::Lt, &s[i+1..])
    } else if let Some(i) = find_op(s, "=") {
        (&s[..i], CompareOp::Eq, &s[i+1..])
    } else if let Some(i) = find_op(s, "#") {
        (&s[..i], CompareOp::Ne, &s[i+1..])
    } else {
        return Err(ParseError(format!("no operator in condition '{s}'")));
    };

    if field.is_empty() {
        return Err(ParseError(format!("empty field in condition '{s}'")));
    }

    let value = parse_value(val_str)?;
    Ok(Condition { field: field.to_string(), op, value })
}

fn find_op(s: &str, op: &str) -> Option<usize> {
    s.find(op)
}

fn parse_value(s: &str) -> Result<CondValue, ParseError> {
    if let Ok(i) = s.parse::<i64>() { return Ok(CondValue::Int(i)); }
    if let Ok(f) = s.parse::<f64>() { return Ok(CondValue::Float(f)); }
    Ok(CondValue::Str(s.to_string()))
}

// ── Condition application ─────────────────────────────────────────────────────

impl Condition {
    pub fn matches_i64(&self, v: i64) -> bool {
        match &self.value {
            CondValue::Int(n) => apply_op_ord(v, *n, self.op),
            CondValue::Float(f) => apply_op_ord(v as f64, *f, self.op),
            CondValue::Str(_) => false,
        }
    }
    pub fn matches_f64(&self, v: f64) -> bool {
        match &self.value {
            CondValue::Int(n) => apply_op_ord(v, *n as f64, self.op),
            CondValue::Float(f) => apply_op_ord(v, *f, self.op),
            CondValue::Str(_) => false,
        }
    }
    pub fn matches_str(&self, v: &str) -> bool {
        match &self.value {
            CondValue::Str(s) => match self.op {
                CompareOp::Eq => v == s.as_str(),
                CompareOp::Ne => v != s.as_str(),
                _ => false,
            },
            _ => false,
        }
    }
}

fn apply_op_ord<T: PartialOrd>(a: T, b: T, op: CompareOp) -> bool {
    match op {
        CompareOp::Gt => a > b,
        CompareOp::Lt => a < b,
        CompareOp::Ge => a >= b,
        CompareOp::Le => a <= b,
        CompareOp::Eq => a == b,
        CompareOp::Ne => a != b,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_sectors() {
        let s = parse_scan_spec("sect *").unwrap();
        assert_eq!(s.object_type, ObjectType::Sector);
        assert!(matches!(s.area, SelectArea::All));
        assert!(s.conditions.is_empty());
    }

    #[test]
    fn parse_by_uid() {
        let s = parse_scan_spec("ship #3").unwrap();
        assert!(matches!(s.area, SelectArea::Uid(3)));
    }

    #[test]
    fn parse_condition_gt() {
        let s = parse_scan_spec("sect * ?eff>50").unwrap();
        assert_eq!(s.conditions.len(), 1);
        let c = &s.conditions[0];
        assert_eq!(c.field, "eff");
        assert_eq!(c.op, CompareOp::Gt);
        assert!(matches!(c.value, CondValue::Int(50)));
    }

    #[test]
    fn parse_condition_ge() {
        let s = parse_scan_spec("nat * ?tech>=100.0").unwrap();
        let c = &s.conditions[0];
        assert_eq!(c.op, CompareOp::Ge);
    }

    #[test]
    fn parse_ne_condition() {
        let s = parse_scan_spec("sect * ?own#0").unwrap();
        let c = &s.conditions[0];
        assert_eq!(c.op, CompareOp::Ne);
    }

    #[test]
    fn condition_matches_i64() {
        let c = Condition { field: "eff".into(), op: CompareOp::Gt,
                            value: CondValue::Int(50) };
        assert!(c.matches_i64(75));
        assert!(!c.matches_i64(30));
    }
}
