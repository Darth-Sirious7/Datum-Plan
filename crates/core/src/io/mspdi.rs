//! Project XML (MSPDI): the XML exchange format many scheduling programs save.
use super::xml::{esc, parse, Node};
use crate::cal::{at, date_of, DAY};
use crate::model::*;
use crate::sched::Sched;
use std::collections::HashMap;
use std::fmt::Write;

fn dt(t: i64) -> String {
    format!("{}T{:02}:{:02}:00", date_of(t), t.rem_euclid(DAY) / 60, t.rem_euclid(60))
}

fn dur(m: i64) -> String {
    format!("PT{}H{}M0S", m / 60, m % 60)
}

/// "PT40H0M0S", "P2DT4H0M0S" → minutes (days count 24 hours, as MSPDI writes elapsed time).
fn read_dur(s: &str) -> i64 {
    let (mut v, mut n, mut time) = (0.0, String::new(), false);
    for ch in s.chars() {
        match ch {
            '0'..='9' | '.' => n.push(ch),
            'T' => time = true,
            'D' => { v += n.parse::<f64>().unwrap_or(0.0) * 1440.0; n.clear() }
            'H' => { v += n.parse::<f64>().unwrap_or(0.0) * 60.0; n.clear() }
            'M' if time => { v += n.parse::<f64>().unwrap_or(0.0); n.clear() }
            'S' => { v += n.parse::<f64>().unwrap_or(0.0) / 60.0; n.clear() }
            _ => n.clear(),
        }
    }
    v.round() as i64
}

fn date(s: &str) -> Option<chrono::NaiveDate> {
    s.get(..10).and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

const CONS: [Constraint; 8] = [Constraint::ASAP, Constraint::ALAP, Constraint::MSO, Constraint::MFO, Constraint::SNET, Constraint::SNLT, Constraint::FNET, Constraint::FNLT];
const LINKS: [LinkType; 4] = [LinkType::FF, LinkType::FS, LinkType::SF, LinkType::SS];

fn unit_code(u: Unit) -> u8 {
    match u { Unit::M => 3, Unit::H => 5, Unit::D => 7, Unit::W => 9, Unit::Mo => 11 }
}

fn hours_xml(out: &mut String, h: &Hours) {
    out.push_str("<WorkingTimes>");
    for (s, e) in h {
        let e = if *e >= 1440 { "00:00:00".to_string() } else { format!("{:02}:{:02}:00", e / 60, e % 60) };
        let _ = write!(out, "<WorkingTime><FromTime>{:02}:{:02}:00</FromTime><ToTime>{e}</ToTime></WorkingTime>", s / 60, s % 60);
    }
    out.push_str("</WorkingTimes>");
}

pub fn write(p: &Project, s: &Sched, cost: &[f64]) -> String {
    let mut o = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Project xmlns=\"http://schemas.microsoft.com/project\">\n");
    let cal_uid = |name: &str| p.calendars.iter().position(|c| c.name == name).map_or(1, |k| k + 1);
    let _ = write!(o, "<SaveVersion>14</SaveVersion><Name>{0}.xml</Name><Title>{0}</Title><ScheduleFromStart>1</ScheduleFromStart><StartDate>{1}</StartDate><FinishDate>{2}</FinishDate>\
<CalendarUID>{3}</CalendarUID><MinutesPerDay>{4}</MinutesPerDay><MinutesPerWeek>{5}</MinutesPerWeek><DaysPerMonth>20</DaysPerMonth><NewTasksAreManual>{6}</NewTasksAreManual>",
        esc(&p.name), dt(s.start), dt(s.finish), cal_uid(&p.calendar), p.day_min, p.day_min * 5, p.manual_default as u8);
    if let Some(st) = p.status {
        let _ = write!(o, "<StatusDate>{}</StatusDate>", dt(at(st) + 17 * 60));
    }
    // Project's "move start of remaining parts before the status date forward": Datum style when on.
    let _ = write!(o, "<MoveRemainingStartsForward>{}</MoveRemainingStartsForward>", !p.opts.msp_status as u8);
    o.push_str("\n<Calendars>");
    for (k, c) in p.calendars.iter().enumerate() {
        let _ = write!(o, "<Calendar><UID>{}</UID><Name>{}</Name><IsBaseCalendar>1</IsBaseCalendar><BaseCalendarUID>-1</BaseCalendarUID><WeekDays>", k + 1, esc(&c.name));
        for (i, h) in c.week.iter().enumerate() {
            let _ = write!(o, "<WeekDay><DayType>{}</DayType><DayWorking>{}</DayWorking>", (i + 1) % 7 + 1, !h.is_empty() as u8);
            if !h.is_empty() {
                hours_xml(&mut o, h);
            }
            o.push_str("</WeekDay>");
        }
        o.push_str("</WeekDays><Exceptions>");
        for e in &c.exceptions {
            let _ = write!(o, "<Exception><EnteredByOccurrences>0</EnteredByOccurrences><TimePeriod><FromDate>{}T00:00:00</FromDate><ToDate>{}T23:59:00</ToDate></TimePeriod>\
<Occurrences>1</Occurrences><Name>{}</Name><Type>1</Type><DayWorking>{}</DayWorking>", e.from, e.to, esc(&e.name), !e.hours.is_empty() as u8);
            if !e.hours.is_empty() {
                hours_xml(&mut o, &e.hours);
            }
            o.push_str("</Exception>");
        }
        o.push_str("</Exceptions></Calendar>");
    }
    // Each person with their own working time: a resource calendar on their base calendar, with their leave.
    let base_uid = |r: &Resource| p.calendars.iter().position(|c| Some(&c.name) == r.cal.as_ref()).or_else(|| p.calendars.iter().position(|c| c.name == p.calendar)).unwrap_or(0) + 1;
    for (k, r) in p.resources.iter().enumerate().filter(|(_, r)| r.kind == ResKind::Work && !r.unassigned) {
        let _ = write!(o, "<Calendar><UID>{}</UID><Name>{}</Name><IsBaseCalendar>0</IsBaseCalendar><BaseCalendarUID>{}</BaseCalendarUID><Exceptions>", 1000 + k, esc(&r.name), base_uid(r));
        for e in &r.exceptions {
            let _ = write!(o, "<Exception><EnteredByOccurrences>0</EnteredByOccurrences><TimePeriod><FromDate>{}T00:00:00</FromDate><ToDate>{}T23:59:00</ToDate></TimePeriod>\
<Occurrences>1</Occurrences><Name>{}</Name><Type>1</Type><DayWorking>{}</DayWorking>", e.from, e.to, esc(&e.name), !e.hours.is_empty() as u8);
            if !e.hours.is_empty() {
                hours_xml(&mut o, &e.hours);
            }
            o.push_str("</Exception>");
        }
        o.push_str("</Exceptions></Calendar>");
    }
    o.push_str("</Calendars>\n<Tasks>");
    let (mut wbs, held): (Vec<u32>, _) = (vec![], super::ext_held(p));
    for (i, t) in p.tasks.iter().enumerate() {
        let lv = t.level as usize;
        wbs.truncate(lv + 1);
        wbs.resize(lv + 1, 0);
        wbs[lv] += 1;
        let num = wbs.iter().map(u32::to_string).collect::<Vec<_>>().join(".");
        let (dn, cons, cdate) = super::held(p, s, i, held[i]);
        let (sum, d) = (s.summary[i], if s.summary[i] { s.cals[s.ci[i]].between(s.es[i], s.ef[i]) } else { dn });
        let _ = write!(o, "<Task><UID>{}</UID><ID>{}</ID><Name>{}</Name><Active>{}</Active><Manual>{}</Manual><Type>0</Type><IsNull>0</IsNull><WBS>{num}</WBS><OutlineNumber>{num}</OutlineNumber>\
<OutlineLevel>{}</OutlineLevel><Start>{}</Start><Finish>{}</Finish><Duration>{}</Duration><DurationFormat>{}</DurationFormat><Milestone>{}</Milestone><Summary>{}</Summary>\
<Critical>{}</Critical><PercentComplete>{}</PercentComplete><PhysicalPercentComplete>{}</PhysicalPercentComplete><EarnedValueMethod>{}</EarnedValueMethod>\
<FixedCost>{}</FixedCost><FixedCostAccrual>{}</FixedCostAccrual><ConstraintType>{}</ConstraintType><CalendarUID>{}</CalendarUID><Cost>{:.2}</Cost>",
            t.uid, i + 1, esc(&t.name), !t.inactive as u8, t.manual as u8, lv + 1, dt(s.es[i]), dt(s.ef[i]), dur(d), unit_code(t.unit) + t.elapsed as u8, (!sum && d == 0) as u8, sum as u8,
            (s.tf[i] <= 0) as u8, t.pct, t.phys, (t.pct_type == PctType::Physical) as u8, t.fixed_cost, match t.accrual { Accrual::Start => 1, Accrual::Prorated => 2, Accrual::End => 3 }, CONS.iter().position(|c| *c == cons).unwrap_or(0), t.cal.as_deref().map_or(-1, |c| cal_uid(c) as i64), cost[i]);
        if let Some(c) = cdate {
            let _ = write!(o, "<ConstraintDate>{}T08:00:00</ConstraintDate>", c);
        }
        if t.ignore_rc {
            o.push_str("<IgnoreResourceCalendar>1</IgnoreResourceCalendar>");
        }
        if let Some(c) = t.deadline {
            let _ = write!(o, "<Deadline>{}T17:00:00</Deadline>", c);
        }
        if let Some(a) = t.act_start {
            let _ = write!(o, "<ActualStart>{}</ActualStart>", dt(at(a) + 8 * 60));
        }
        if let Some(a) = t.act_finish {
            let _ = write!(o, "<ActualFinish>{}</ActualFinish>", dt(at(a) + 17 * 60));
        }
        if !t.notes.is_empty() {
            let _ = write!(o, "<Notes>{}</Notes>", esc(&t.notes));
        }
        if !t.code.is_empty() {
            let _ = write!(o, "<Text1>{}</Text1>", esc(&t.code));
        }
        for l in &t.preds {
            let _ = write!(o, "<PredecessorLink><PredecessorUID>{}</PredecessorUID><Type>{}</Type><CrossProject>0</CrossProject><LinkLag>{}</LinkLag><LagFormat>{}</LagFormat></PredecessorLink>",
                l.pred, LINKS.iter().position(|k| *k == l.kind).unwrap(), l.lag * 10, if l.pct { 19 } else if l.elapsed { 8 } else { 7 });
        }
        for (n, b) in &t.bls {
            let _ = write!(o, "<Baseline><Number>{n}</Number><Start>{}</Start><Finish>{}</Finish><Duration>{}</Duration><Cost>{:.2}</Cost>{}</Baseline>", dt(b.start), dt(b.finish), dur(b.dur), b.cost, b.work.map_or(String::new(), |w| format!("<Work>{}</Work>", dur(w.round() as i64))));
        }
        o.push_str("</Task>\n");
    }
    o.push_str("</Tasks>\n<Resources>");
    // Project's own unassigned resource (UID -65535) is implied: it is not written.
    for (k, r) in p.resources.iter().enumerate().filter(|(_, r)| !r.unassigned) {
        let ty = match r.kind { ResKind::Work => 1, ResKind::Material => 0, ResKind::Cost => 2 };
        let _ = write!(o, "<Resource><UID>{}</UID><ID>{}</ID><Name>{}</Name><Type>{ty}</Type><IsNull>0</IsNull><Group>{}</Group><MaterialLabel>{}</MaterialLabel>\
<MaxUnits>{:.2}</MaxUnits><StandardRate>{}</StandardRate><StandardRateFormat>2</StandardRateFormat><CostPerUse>{}</CostPerUse>{}</Resource>",
            r.uid, k + 1, esc(&r.name), esc(&r.group), esc(&r.label), r.max, r.rate, r.per_use, if r.kind == ResKind::Work { format!("<CalendarUID>{}</CalendarUID>", 1000 + k) } else { String::new() });
    }
    o.push_str("</Resources>\n<Assignments>");
    let mut n = 1;
    for t in &p.tasks {
        for a in &t.res {
            let none = p.res_idx(a.res).is_some_and(|k| p.resources[k].unassigned);
            let _ = write!(o, "<Assignment><UID>{n}</UID><TaskUID>{}</TaskUID><ResourceUID>{}</ResourceUID><Units>{}</Units><Work>{}</Work></Assignment>",
                t.uid, if none { "-65535".to_string() } else { a.res.to_string() }, if none { 1.0 } else { a.units }, dur((t.dur as f64 * a.units).round() as i64));
            n += 1;
        }
    }
    o.push_str("</Assignments>\n</Project>\n");
    o
}

fn read_hours(n: &Node) -> Hours {
    n.get("WorkingTimes").map_or(vec![], |w| w.all("WorkingTime").filter_map(|t| {
        let m = |s: &str| -> Option<u16> { Some(s.get(..2)?.parse::<u16>().ok()? * 60 + s.get(3..5)?.parse::<u16>().ok()?) };
        let (a, b) = (m(t.val("FromTime"))?, m(t.val("ToTime"))?);
        Some((a, if b == 0 { 1440 } else { b }))
    }).collect())
}

pub fn read(text: &str) -> Result<Project, String> { read_notes(text).map(|x| x.0) }

/// Reads a Project XML file and notes what was changed or left out on the way in.
pub fn read_notes(text: &str) -> Result<(Project, Vec<String>), String> {
    let (mut notes, mut elapsed, mut late_bl) = (vec![], 0, 0);
    let root = parse(text)?;
    if root.name != "Project" {
        return Err("This XML file is not a project XML file.".into());
    }
    let start = date(root.val("StartDate")).unwrap_or_else(|| chrono::Local::now().date_naive());
    let name = [root.val("Title"), root.val("Name").trim_end_matches(".xml")].into_iter().find(|s| !s.is_empty()).unwrap_or("Imported project");
    let mut p = Project::new(name, start);
    p.status = date(root.val("StatusDate"));
    p.day_min = root.val("MinutesPerDay").parse().unwrap_or(480);
    p.manual_default = root.val("NewTasksAreManual") == "1";
    let mut cal_names: HashMap<String, String> = HashMap::new();
    let mut res_cals: HashMap<String, (Option<String>, Vec<Exception>)> = HashMap::new();
    if let Some(cs) = root.get("Calendars") {
        let mut cals = vec![];
        for c in cs.all("Calendar").filter(|c| c.val("IsBaseCalendar") != "0") {
            let mut cal = Calendar::standard(c.val("Name"));
            if cal.name.is_empty() || cals.iter().any(|x: &Calendar| x.name == cal.name) {
                cal.name = format!("Calendar {}", c.val("UID"));
            }
            for wd in c.get("WeekDays").into_iter().flat_map(|w| w.all("WeekDay")) {
                let ty: usize = wd.val("DayType").parse().unwrap_or(0);
                let h = if wd.val("DayWorking") == "1" { read_hours(wd) } else { vec![] };
                if (1..=7).contains(&ty) {
                    cal.week[(ty + 5) % 7] = if h.is_empty() && wd.val("DayWorking") == "1" { STD_HOURS.to_vec() } else { h };
                } else if ty == 0 {
                    if let Some(tp) = wd.get("TimePeriod") {
                        if let (Some(f), Some(t)) = (date(tp.val("FromDate")), date(tp.val("ToDate"))) {
                            cal.exceptions.push(Exception { name: "Exception".into(), from: f, to: t, hours: read_hours(wd) });
                        }
                    }
                }
            }
            for e in c.get("Exceptions").into_iter().flat_map(|x| x.all("Exception")) {
                if let Some(tp) = e.get("TimePeriod") {
                    if let (Some(f), Some(t)) = (date(tp.val("FromDate")), date(tp.val("ToDate"))) {
                        let h = if e.val("DayWorking") == "1" { read_hours(e) } else { vec![] };
                        cal.exceptions.push(Exception { name: e.val("Name").to_string(), from: f, to: t, hours: h });
                    }
                }
            }
            cal_names.insert(c.val("UID").to_string(), cal.name.clone());
            cals.push(cal);
        }
        // Resource calendars: a base calendar and the person's own exceptions (leave). One with
        // its own working week becomes a calendar of its own.
        for c in cs.all("Calendar").filter(|c| c.val("IsBaseCalendar") == "0") {
            let exc: Vec<Exception> = c.get("Exceptions").into_iter().flat_map(|x| x.all("Exception")).filter_map(|e| {
                let tp = e.get("TimePeriod")?;
                Some(Exception { name: e.val("Name").to_string(), from: date(tp.val("FromDate"))?, to: date(tp.val("ToDate"))?, hours: if e.val("DayWorking") == "1" { read_hours(e) } else { vec![] } })
            }).collect();
            let base = cal_names.get(c.val("BaseCalendarUID")).cloned();
            let week: Vec<_> = c.get("WeekDays").into_iter().flat_map(|w| w.all("WeekDay")).filter(|wd| (1..=7).contains(&wd.val("DayType").parse::<usize>().unwrap_or(0))).collect();
            if week.is_empty() {
                res_cals.insert(c.val("UID").to_string(), (base, exc));
            } else {
                let mut cal = base.and_then(|b| cals.iter().find(|x: &&Calendar| x.name == b).cloned()).unwrap_or_else(|| Calendar::standard(""));
                cal.name = format!("{} (own)", c.val("Name"));
                for wd in week {
                    let ty: usize = wd.val("DayType").parse().unwrap_or(1);
                    cal.week[(ty + 5) % 7] = if wd.val("DayWorking") == "1" { let h = read_hours(wd); if h.is_empty() { STD_HOURS.to_vec() } else { h } } else { vec![] };
                }
                cal_names.insert(c.val("UID").to_string(), cal.name.clone());
                res_cals.insert(c.val("UID").to_string(), (Some(cal.name.clone()), exc));
                cals.push(cal);
            }
        }
        if !cals.is_empty() {
            p.calendars = cals;
            if let Some(n) = cal_names.get(root.val("CalendarUID")) {
                p.calendar = n.clone();
            }
        }
    }
    let unit = |c: &str| match c { "3" | "4" => Unit::M, "5" | "6" => Unit::H, "9" | "10" => Unit::W, "11" | "12" => Unit::Mo, _ => Unit::D };
    let mut tasks: Vec<(i64, Task)> = vec![];
    let mut task_work: HashMap<u32, i64> = HashMap::new();
    for t in root.get("Tasks").into_iter().flat_map(|x| x.all("Task")) {
        let uid: u32 = t.val("UID").parse().unwrap_or(0);
        if uid == 0 || t.val("IsNull") == "1" || t.val("OutlineLevel") == "0" {
            continue;
        }
        let mut k = Task {
            uid, name: t.val("Name").to_string(), ignore_rc: t.val("IgnoreResourceCalendar") == "1",
            level: t.val("OutlineLevel").parse::<u8>().unwrap_or(1).saturating_sub(1),
            dur: read_dur(t.val("Duration")), unit: unit(t.val("DurationFormat")), elapsed: matches!(t.val("DurationFormat"), "4" | "6" | "8" | "10" | "12"),
            phys: t.val("PhysicalPercentComplete").parse().unwrap_or(0), pct_type: if t.val("EarnedValueMethod") == "1" { PctType::Physical } else { PctType::Duration },
            fixed_cost: t.val("FixedCost").parse().unwrap_or(0.0), accrual: match t.val("FixedCostAccrual") { "1" => Accrual::Start, "3" => Accrual::End, _ => Accrual::Prorated },
            pct: t.val("PercentComplete").parse().unwrap_or(0),
            constraint: t.val("ConstraintType").parse::<usize>().ok().and_then(|i| CONS.get(i).copied()).unwrap_or_default(),
            cdate: date(t.val("ConstraintDate")), deadline: date(t.val("Deadline")),
            manual: t.val("Manual") == "1", inactive: t.val("Active") == "0",
            cal: cal_names.get(t.val("CalendarUID")).cloned(),
            act_start: date(t.val("ActualStart")), act_finish: date(t.val("ActualFinish")),
            code: t.val("Text1").to_string(), notes: t.get("Notes").map_or(String::new(), |n| n.text.clone()),
            ..Default::default()
        };
        if k.manual {
            k.mstart = date(t.val("Start"));
        }
        if matches!(k.constraint, Constraint::ASAP | Constraint::ALAP) {
            k.cdate = None;
        }
        for l in t.all("PredecessorLink") {
            if let Ok(pred) = l.val("PredecessorUID").parse() {
                let lag = l.val("LinkLag").parse::<i64>().unwrap_or(0) / 10;
                let pct = matches!(l.val("LagFormat"), "19" | "20");
                // An elapsed lag stays clock time; an elapsed percent lag is kept as a percent of the working duration.
                elapsed += (l.val("LagFormat") == "20") as usize;
                k.preds.push(Link { pred, kind: l.val("Type").parse::<usize>().ok().and_then(|i| LINKS.get(i).copied()).unwrap_or(LinkType::FS), lag, pct, elapsed: matches!(l.val("LagFormat"), "4" | "6" | "8" | "10" | "12") });
            }
        }
        for b in t.all("Baseline") {
            let Ok(n) = b.val("Number").parse::<u8>() else { continue };
            if n > 10 { late_bl += 1 }
            if let (Some(bs), Some(bf), true) = (date(b.val("Start")), date(b.val("Finish")), n <= 10) {
                let tm = |s: &str| s.get(11..16).and_then(|x| Some(x.get(..2)?.parse::<i64>().ok()? * 60 + x.get(3..)?.parse::<i64>().ok()?)).unwrap_or(0);
                k.bls.insert(n, Baseline { start: at(bs) + tm(b.val("Start")), finish: at(bf) + tm(b.val("Finish")), dur: read_dur(b.val("Duration")), cost: b.val("Cost").parse().unwrap_or(0.0), work: (!b.val("Work").is_empty()).then(|| read_dur(b.val("Work")) as f64) });
            }
        }
        if !t.val("Work").is_empty() {
            task_work.insert(k.uid, read_dur(t.val("Work")));
        }
        tasks.push((t.val("ID").parse().unwrap_or(i64::MAX), k));
    }
    tasks.sort_by_key(|(id, _)| *id);
    p.tasks = tasks.into_iter().map(|(_, t)| t).collect();
    for r in root.get("Resources").into_iter().flat_map(|x| x.all("Resource")) {
        let uid: u32 = r.val("UID").parse().unwrap_or(0);
        if uid == 0 || r.val("IsNull") == "1" || r.val("Name").is_empty() {
            continue;
        }
        p.resources.push(Resource {
            uid, name: r.val("Name").into(), group: r.val("Group").into(), label: r.val("MaterialLabel").into(),
            kind: match r.val("Type") { "0" => ResKind::Material, "2" => ResKind::Cost, _ => ResKind::Work },
            max: r.val("MaxUnits").parse().unwrap_or(1.0), rate: r.val("StandardRate").parse().unwrap_or(0.0), per_use: r.val("CostPerUse").parse().unwrap_or(0.0),
            initials: r.val("Initials").into(), email: r.val("EmailAddress").into(), ovt: r.val("OvertimeRate").parse().unwrap_or(0.0),
            cal: res_cals.get(r.val("CalendarUID")).and_then(|x| x.0.clone()).or_else(|| cal_names.get(r.val("CalendarUID")).cloned()).filter(|c| *c != p.calendar),
            exceptions: res_cals.get(r.val("CalendarUID")).map(|x| x.1.clone()).unwrap_or_default(),
            ..Default::default()
        });
    }
    // Project XML files always schedule on resource calendars: keep that when people have their own.
    if p.resources.iter().any(|r| r.cal.is_some() || !r.exceptions.is_empty()) {
        p.opts.res_cals = true;
    }
    let idx: std::collections::HashMap<u32, usize> = p.tasks.iter().enumerate().rev().map(|(i, t)| (t.uid, i)).collect();
    // Work typed on a task with no resource is assigned to Project's "unassigned" resource (-65535).
    let mut loose: Vec<(usize, i64)> = vec![];
    for a in root.get("Assignments").into_iter().flat_map(|x| x.all("Assignment")) {
        let tu: u32 = a.val("TaskUID").parse().unwrap_or(0);
        let Some(&i) = idx.get(&tu) else { continue };
        match a.val("ResourceUID").parse::<u32>() {
            Ok(ru) if p.resources.iter().any(|r| r.uid == ru) => p.tasks[i].res.push(Assign { res: ru, units: a.val("Units").parse().unwrap_or(1.0), table: 0 }),
            // Project also writes one for a task with no work; the task's own Work is the truth.
            _ => loose.push((i, read_dur(a.val("Work")).min(task_work.get(&tu).copied().unwrap_or(i64::MAX)))),
        }
    }
    // Keep those hours: one work resource whose units on each task give the task's work back.
    let loose: Vec<(usize, f64)> = loose.into_iter().filter(|&(i, w)| w > 0 && p.tasks[i].dur > 0).map(|(i, w)| (i, w as f64 / p.tasks[i].dur as f64)).collect();
    if !loose.is_empty() {
        let uid = p.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1;
        p.resources.push(Resource { uid, name: "Project work (no resource)".into(), kind: ResKind::Work, max: 100.0, unassigned: true, ..Default::default() });
        for &(i, u) in &loose {
            p.tasks[i].res.push(Assign { res: uid, units: (u * 10_000.0).round() / 10_000.0, table: 0 });
        }
        notes.push(format!("{} tasks have work with no resource; it is kept on the work resource \"Project work (no resource)\", which goes back to Project as work with no resource.", loose.len()));
    }
    // Unless the file asks for remaining work to move forward to the status date (Project's
    // calculation option, which Datum Plan writes), the status date measures progress only.
    p.opts.msp_status = root.val("MoveRemainingStartsForward") != "1";
    // Project's flag moves only the remaining parts of tasks under way; Datum style also holds
    // unstarted work back, so the two match when the flag is off and differ a little when on.
    let uids: std::collections::HashSet<u32> = p.tasks.iter().map(|t| t.uid).collect();
    let missing: usize = p.tasks.iter_mut().map(|t| { let n = t.preds.len(); t.preds.retain(|l| uids.contains(&l.pred)); n - t.preds.len() }).sum();
    if missing > 0 { notes.push(format!("{missing} links point to tasks that are not in the file; they were left out.")) }
    if elapsed > 0 { notes.push(format!("{elapsed} links have an elapsed percent lag; Datum Plan counts each as that percent of the predecessor's working duration, on the lag calendar.")) }
    if late_bl > 0 { notes.push(format!("{late_bl} baseline values beyond Baseline 10 were left out.")) }
    p.normalize();
    Ok((p, notes))
}
