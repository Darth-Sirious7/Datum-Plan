//! Text in and out of cells: durations, dates, predecessors, resources and quick-add sentences.
use crate::model::{Assign, Constraint, Link, LinkType, Project, ResKind, Unit, CONSTRAINTS};
use chrono::NaiveDate;
use std::collections::HashMap;

/// Working minutes per unit, given the minutes in a working day.
pub fn unit_min(u: Unit, day: i64) -> i64 {
    let day = day.max(1);
    match u { Unit::M => 1, Unit::H => 60, Unit::D => day, Unit::W => 5 * day, Unit::Mo => 20 * day }
}

/// Clock minutes per unit of an elapsed duration.
fn clock(u: Unit) -> i64 { match u { Unit::M => 1, Unit::H => 60, Unit::D => 1440, Unit::W => 10080, Unit::Mo => 43200 } }

/// Like `dur`, but also accepts elapsed durations ("3ed", "12eh"): returns clock minutes and true.
pub fn dur_any(s: &str, day: i64) -> Result<(i64, Unit, bool), String> {
    let t = s.trim().to_lowercase();
    let cut = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ' ')).unwrap_or(t.len());
    if t[cut..].starts_with('e') && t.len() > cut + 1 {
        let (m, u) = dur(&format!("{}{}", &t[..cut], &t[cut + 1..]), day)?;
        return Ok(((m as f64 / unit_min(u, day) as f64 * clock(u) as f64).round() as i64, u, true));
    }
    dur(&t, day).map(|(m, u)| (m, u, false))
}

/// A part of a task's duration as typed (remaining, actual): on an elapsed task "1d" is one elapsed day.
pub fn dur_for(s: &str, day: i64, elapsed: bool) -> Result<i64, String> {
    let (m, u, e) = dur_any(s, day)?;
    Ok(if elapsed && !e { (m as f64 / unit_min(u, day) as f64 * clock(u) as f64).round() as i64 } else { m })
}

/// Shows an elapsed duration ("3 edays").
pub fn fmt_edur(min: i64, u: Unit) -> String {
    let v = min as f64 / clock(u) as f64;
    let name = match u { Unit::M => "emins", Unit::H => "ehrs", Unit::D => "edays", Unit::W => "ewks", Unit::Mo => "emons" };
    format!("{} {name}", num(v))
}

/// "5", "5d", "4h", "2 wks", "1.5w", "1 mo" → working minutes and the unit typed.
pub fn dur(s: &str, day: i64) -> Result<(i64, Unit), String> {
    let s = s.trim().to_lowercase();
    let cut = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let (num, unit) = s.split_at(cut);
    let v: f64 = num.parse().map_err(|_| format!("\"{s}\" is not a duration. Try 5d, 4h, 2w or 1mo."))?;
    let u = match unit.trim().trim_end_matches('?') {
        "m" | "min" | "mins" | "minute" | "minutes" => Unit::M,
        "h" | "hr" | "hrs" | "hour" | "hours" => Unit::H,
        "" | "d" | "day" | "days" | "dy" | "dys" => Unit::D,
        "w" | "wk" | "wks" | "week" | "weeks" => Unit::W,
        "mo" | "mon" | "mons" | "month" | "months" => Unit::Mo,
        u => return Err(format!("Unknown duration unit \"{u}\". Use h, d, w or mo.")),
    };
    let m = (v * unit_min(u, day) as f64).round();
    if !(0.0..=crate::model::MAX_SPAN as f64).contains(&m) {
        return Err("Duration is out of range (at most about 80 years).".into());
    }
    Ok((m as i64, u))
}

fn num(v: f64) -> String {
    // Halves round up (54.625 → 54.63), as MS Project shows them; plain {:.2} rounds them to even.
    let s = format!("{:.2}", (v * 100.0).round() / 100.0);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn fmt_dur(min: i64, u: Unit, day: i64) -> String {
    let v = min as f64 / unit_min(u, day) as f64;
    let one = (v - 1.0).abs() < 1e-9;
    let name = match u { Unit::M => "min", Unit::H => if one { "hr" } else { "hrs" }, Unit::D => if one { "day" } else { "days" }, Unit::W => if one { "wk" } else { "wks" }, Unit::Mo => if one { "mon" } else { "mons" } };
    format!("{} {name}", num(v))
}

/// Short form for lags: "+2d", "-4h".
pub fn fmt_lag(min: i64, day: i64) -> String { fmt_lag_el(min, day, false) }

/// Short form for a lag that may be elapsed (clock minutes): "+7ed", "-12eh", "+30em".
pub fn fmt_lag_el(min: i64, day: i64, elapsed: bool) -> String {
    if min == 0 {
        return String::new();
    }
    let (day, e) = if elapsed { (crate::cal::DAY, "e") } else { (day.max(1), "") };
    let (v, u) = if min % day == 0 { (min / day, "d") } else if min % 60 == 0 { (min / 60, "h") } else { (min, "m") };
    format!("{}{v}{e}{u}", if min > 0 { "+" } else { "" })
}

const DATE_FORMATS: [&str; 9] = ["%Y-%m-%d", "%d-%b-%y", "%d-%b-%Y", "%d/%m/%Y", "%d/%m/%y", "%d %b %Y", "%d %b %y", "%a %d-%b-%y", "%Y-%m-%dT%H:%M:%S"];

/// Parses a date in any common form; `fmt` (the display format) is tried first. A year typed with
/// two digits is read as %y reads it (26 → 2026, 99 → 1999), even where a format asks for four.
pub fn date_in(s: &str, fmt: &str) -> Result<NaiveDate, String> {
    use chrono::Datelike;
    let s = s.trim();
    let s = s.get(..19).filter(|_| s.contains('T')).unwrap_or(s);
    let d = std::iter::once(fmt)
        .chain(DATE_FORMATS)
        .find_map(|f| NaiveDate::parse_from_str(s, f).ok().or_else(|| chrono::NaiveDateTime::parse_from_str(s, f).ok().map(|d| d.date())))
        .ok_or_else(|| format!("\"{s}\" is not a date. Try 24-Oct-26 or 2026-10-24."))?;
    let short = !s.split(|c: char| !c.is_ascii_digit()).any(|w| w.len() > 2);
    let d = match d.year() { y @ 0..=68 if short => d.with_year(2000 + y), y @ 69..=99 if short => d.with_year(1900 + y), _ => Some(d) }.unwrap_or(d);
    if d < crate::cal::lo() || d > crate::cal::hi() {
        return Err(format!("\"{s}\" is outside the dates a plan can hold (1800 to 2600)."));
    }
    Ok(d)
}

pub fn date(s: &str) -> Result<NaiveDate, String> {
    date_in(s, "%d-%b-%y")
}

/// "3, 5SS+2d, 7FF-4h" with row IDs (1-based) → links. `ids[i]` is the uid of row i+1.
pub fn preds(s: &str, ids: &[u32], day: i64) -> Result<Vec<Link>, String> {
    let mut out = vec![];
    for tok in s.split([',', ';']).map(str::trim).filter(|t| !t.is_empty()) {
        let t = tok.to_uppercase().replace(' ', "");
        let cut = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
        let id: usize = t[..cut].parse().map_err(|_| format!("\"{tok}\" should start with a task ID, like 3 or 3SS+2d."))?;
        let uid = *ids.get(id.wrapping_sub(1)).ok_or_else(|| format!("There is no task {id}."))?;
        let mut rest = &t[cut..];
        let kind = match rest.get(..2) {
            Some("SS") => LinkType::SS,
            Some("FF") => LinkType::FF,
            Some("SF") => LinkType::SF,
            _ => LinkType::FS,
        };
        if matches!(rest.get(..2), Some("FS" | "SS" | "FF" | "SF")) {
            rest = &rest[2..];
        }
        let (lag, pct, elapsed) = match rest.chars().next() {
            None => (0, false, false),
            Some('+' | '-') => lag(rest, day).map_err(|e| format!("\"{tok}\": {e}"))?,
            _ => return Err(format!("Write the lag in \"{tok}\" as +2d, -4h, +7ed or +50%.")),
        };
        out.push(Link { pred: uid, kind, lag, pct, elapsed });
    }
    Ok(out)
}

/// A lag as typed: "+2d" or "2d" waits, "-4h" overlaps, "+7ed" or "-12eh" is elapsed (clock time,
/// round the clock), "50%" or "-20%" is a share of the predecessor's duration. Empty is no lag.
/// Gives working minutes (clock minutes if elapsed, or the percent), whether it is a percent and whether elapsed.
pub fn lag(s: &str, day: i64) -> Result<(i64, bool, bool), String> {
    let t: String = s.replace('\u{2212}', "-").chars().filter(|c| !c.is_whitespace()).collect();
    let (neg, x) = match t.strip_prefix('-') { Some(x) => (true, x), None => (false, t.strip_prefix('+').unwrap_or(&t)) };
    if x.is_empty() { return Ok((0, false, false)) }
    let (pct, mut el) = (x.ends_with('%'), false);
    let v = if pct {
        x.trim_end_matches('%').parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.clamp(-10_000.0, 10_000.0).round() as i64).ok_or("write a percent lag as +50%.")?
    } else { let (m, _, e) = dur_any(x, day)?; el = e; m };
    Ok((if neg { -v } else { v }, pct, el))
}

pub fn fmt_link(id: usize, l: &Link, day: i64) -> String {
    let k = if l.kind == LinkType::FS && l.lag == 0 { String::new() } else { format!("{:?}", l.kind) };
    let lag = if l.pct && l.lag != 0 { format!("{:+}%", l.lag) } else { fmt_lag_el(l.lag, day, l.elapsed) };
    format!("{id}{k}{lag}")
}

pub fn fmt_preds(links: &[Link], id_of: &HashMap<u32, usize>, day: i64) -> String {
    links.iter().filter_map(|l| Some(fmt_link(*id_of.get(&l.pred)?, l, day))).collect::<Vec<_>>().join(", ")
}

/// "Engineer[50%], Concrete[12], Permit fee[4500]" → assignments; unknown names become work resources.
pub fn res(s: &str, p: &mut Project) -> Result<Vec<Assign>, String> {
    let mut out: Vec<Assign> = vec![];
    for tok in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let (name, amt) = match tok.find('[') {
            Some(i) => (tok[..i].trim(), Some(tok[i + 1..].trim_end_matches(']').trim())),
            None => (tok, None),
        };
        let uid = p.resource(name);
        let r = &p.resources[p.res_idx(uid).unwrap()];
        let units = match amt {
            None => if r.kind == ResKind::Work { 1.0 } else { 0.0 },
            Some(a) => {
                let pct = a.ends_with('%');
                let v: f64 = a.trim_end_matches('%').split_whitespace().next().unwrap_or("").trim_start_matches(['$', '€', '£', '₹']).replace(',', "").parse()
                    .map_err(|_| format!("\"{tok}\": write the amount as Name[50%] or Name[12]."))?;
                if pct || r.kind == ResKind::Work { v / if pct || v > 10.0 { 100.0 } else { 1.0 } } else { v }
            }
        };
        out.retain(|a| a.res != uid);
        out.push(Assign { res: uid, units, table: 0 });
    }
    Ok(out)
}

pub fn fmt_res(a: &[Assign], p: &Project) -> String {
    a.iter()
        .filter_map(|a| {
            let r = &p.resources[p.res_idx(a.res)?];
            Some(match r.kind {
                ResKind::Work if (a.units - 1.0).abs() < 1e-9 => r.name.clone(),
                ResKind::Work => format!("{}[{}%]", r.name, num(a.units * 100.0)),
                _ if r.label.is_empty() => format!("{}[{}]", r.name, num(a.units)),
                _ => format!("{}[{} {}]", r.name, num(a.units), r.label),
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn constraint(s: &str) -> Result<Constraint, String> {
    let s = s.trim();
    CONSTRAINTS
        .iter()
        .find(|(c, n)| n.eq_ignore_ascii_case(s) || format!("{c:?}").eq_ignore_ascii_case(s))
        .map(|(c, _)| *c)
        .ok_or_else(|| format!("\"{s}\" is not a constraint type. Try As Soon As Possible or SNET."))
}

/// A one-line task: "Pour slab 5d after 4, 6 @Crew[50%] start 2-Nov-26 by 20-Nov-26".
/// Returns (field, value) pairs for a new task.
pub fn quick(text: &str) -> Vec<(String, String)> {
    let mut f: Vec<(String, String)> = vec![];
    let mut name = vec![];
    let mut res = vec![];
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    let take = |i: &mut usize, stop: &[&str]| {
        let mut v = vec![];
        while *i < words.len() && !stop.contains(&words[*i].to_lowercase().as_str()) && !words[*i].starts_with('@') && !is_dur(words[*i]) {
            v.push(words[*i]);
            *i += 1;
        }
        v.join(" ")
    };
    const KEYS: [&str; 5] = ["after", "by", "start", "on", "from"];
    while i < words.len() {
        let w = words[i];
        let lw = w.to_lowercase();
        i += 1;
        if let Some(r) = w.strip_prefix('@') {
            // A name runs to the next keyword, @ or duration, or to a closing "]".
            let mut v = vec![r];
            while !r.contains('[') && i < words.len() && !words[i].starts_with('@') && !KEYS.contains(&words[i].to_lowercase().as_str()) && !is_dur(words[i]) {
                v.push(words[i]);
                i += 1;
                if words[i - 1].ends_with(']') {
                    break;
                }
            }
            res.push(v.join(" "));
        } else if lw == "after" {
            f.push(("preds".into(), take(&mut i, &KEYS)));
        } else if lw == "by" {
            f.push(("deadline".into(), take(&mut i, &KEYS)));
        } else if lw == "start" || lw == "on" || lw == "from" {
            f.push(("start".into(), take(&mut i, &KEYS)));
        } else if is_dur(w) {
            f.push(("dur".into(), w.into()));
        } else if lw == "milestone" {
            f.push(("dur".into(), "0".into()));
        } else {
            name.push(w);
        }
    }
    if !res.is_empty() {
        f.push(("res".into(), res.join(", ")));
    }
    f.insert(0, ("name".into(), name.join(" ")));
    f
}

fn is_dur(w: &str) -> bool {
    w.starts_with(|c: char| c.is_ascii_digit()) && w.contains(|c: char| c.is_alphabetic()) && dur(w, 480).is_ok()
}
