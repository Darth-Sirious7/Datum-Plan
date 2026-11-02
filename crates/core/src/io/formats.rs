//! Export-only formats: JSON, HTML, iCalendar, MPX and PMXML.
use super::xml::esc;
use crate::cal::{date_of, DAY};
use crate::fields::Calc;
use crate::model::*;
use std::fmt::Write;

/// Plain JSON of every task with its main calculated fields, for scripts and other tools.
pub fn json(c: &Calc, cols: &[String]) -> String {
    let keys: Vec<String> = cols.iter().filter_map(|k| c.key_of(k)).collect();
    let tasks: Vec<serde_json::Value> = (0..c.p.tasks.len()).map(|i| {
        let mut m = serde_json::Map::new();
        m.insert("uid".into(), c.p.tasks[i].uid.into());
        m.insert("summary".into(), c.s.summary[i].into());
        m.insert("level".into(), (c.p.tasks[i].level + 1).into());
        for k in &keys {
            m.insert(c.name(k), c.text(i, k).into());
        }
        serde_json::Value::Object(m)
    }).collect();
    let s = c.summary();
    serde_json::to_string_pretty(&serde_json::json!({ "project": c.p.name, "start": s.start, "finish": s.finish, "currency": c.p.currency.code, "tasks": tasks })).unwrap_or_default()
}

/// A self-contained HTML report: header with logo, project facts and the task table.
pub fn html(c: &Calc, cols: &[String]) -> String {
    let p = c.p;
    let keys: Vec<String> = cols.iter().filter_map(|k| c.key_of(k)).collect();
    let s = c.summary();
    let mut o = String::from("<!doctype html><html><head><meta charset=\"utf-8\"><title>");
    o.push_str(&esc(&p.name));
    o.push_str("</title><style>body{font:13px Segoe UI,Arial,sans-serif;color:#1f2933;margin:24px}header{display:flex;gap:16px;align-items:center;border-bottom:2px solid #2563eb;padding-bottom:10px}\
header img{max-height:48px}h1{font-size:20px;margin:0}.facts{display:flex;gap:28px;margin:14px 0;color:#52606d}.facts b{display:block;color:#1f2933;font-size:15px}\
table{border-collapse:collapse;width:100%}th{background:#eaf1fd;text-align:left;font-weight:600}th,td{border:1px solid #d9e2ec;padding:3px 6px;white-space:nowrap}\
tr.sum td{font-weight:600;background:#f5f8fc}tr.crit td{color:#c81e1e}footer{margin-top:12px;color:#829ab1;font-size:11px}</style></head><body><header>");
    if !p.logo.is_empty() {
        let _ = write!(o, "<img src=\"{}\" alt=\"\">", esc(&p.logo));
    }
    let _ = write!(o, "<div><h1>{}</h1><div>{}</div></div></header><div class=\"facts\">", esc(&p.name), esc(&[p.company.as_str(), p.client.as_str()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" · ")));
    for (k, v) in [("Start", &s.start), ("Finish", &s.finish), ("Duration", &s.dur), ("Cost", &s.cost), ("Status date", &s.status)] {
        if !v.is_empty() {
            let _ = write!(o, "<div>{k}<b>{}</b></div>", esc(v));
        }
    }
    let _ = write!(o, "<div>Complete<b>{:.0}%</b></div></div><table><tr>", s.pct);
    keys.iter().for_each(|k| { let _ = write!(o, "<th>{}</th>", esc(&c.name(k))); });
    o.push_str("</tr>");
    for i in 0..p.tasks.len() {
        let cls = if c.s.summary[i] { " class=\"sum\"" } else if c.s.critical(p, i) { " class=\"crit\"" } else { "" };
        let _ = write!(o, "<tr{cls}>");
        for k in &keys {
            let pad = if k == "name" { format!(" style=\"padding-left:{}px\"", 6 + p.tasks[i].level as usize * 16) } else { String::new() };
            let _ = write!(o, "<td{pad}>{}</td>", esc(&c.text(i, k)));
        }
        o.push_str("</tr>");
    }
    let _ = write!(o, "</table><footer>Made with Datum Plan · {}</footer></body></html>", c.today);
    o
}

/// iCalendar: tasks and milestones as all-day events.
pub fn ics(c: &Calc) -> String {
    let p = c.p;
    let mut o = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Datum Plan//EN\r\nCALSCALE:GREGORIAN\r\n");
    let fold = |s: &str| s.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;").replace('\r', "").replace('\n', "\\n");
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !c.s.summary[i] && !c.s.off[i]) {
        let a = date_of(c.s.es[i]);
        let b = date_of((c.s.ef[i] - 1).max(c.s.es[i])).succ_opt().unwrap_or(a);
        let _ = writeln!(o, "BEGIN:VEVENT\r\nUID:task-{}@datumplan\r\nDTSTAMP:{}T000000Z\r\nDTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\nSUMMARY:{}\r\nDESCRIPTION:{}\r\nEND:VEVENT\r",
            t.uid, c.today.format("%Y%m%d"), a.format("%Y%m%d"), b.format("%Y%m%d"), fold(&format!("{}{}", if t.dur == 0 { "◆ " } else { "" }, t.name)), fold(&format!("{} · {}% complete · {}", p.name, t.pct, c.text(i, "res"))));
    }
    o.push_str("END:VCALENDAR\r\n");
    o
}

fn mpx_date(t: i64) -> String { date_of(t).format("%m/%d/%Y").to_string() }

/// MPX 4.0, the old exchange format many tools still read.
pub fn mpx(c: &Calc) -> String {
    let (p, s) = (c.p, &c.s);
    let q = |x: &str| if x.contains([',', '"']) { format!("\"{}\"", x.replace('"', "\"\"")) } else { x.to_string() };
    let mut o = String::from("MPX,Datum Plan,4.0,ANSI\r\n");
    let _ = writeln!(o, "10,$,0,2,0,0,0,0,0,0\r\n11,0,0,1,0,0,0,0,0\r\n12,0,0,{},{},20,0,0\r", p.day_min / 60, p.day_min * 5 / 60);
    for cal in &p.calendars {
        let days: Vec<&str> = (0..7).map(|k| if cal.week[(k + 6) % 7].is_empty() { "0" } else { "1" }).collect();
        let _ = writeln!(o, "20,{},{}\r", q(&cal.name), days.join(","));
    }
    let _ = writeln!(o, "30,{},,,{},{}\r", q(&p.name), mpx_date(s.start), mpx_date(s.finish));
    // Field numbers as MPX defines them: 40 ID, 49 Unique ID, 1 Name, 42 Standard Rate, 44 Cost Per Use.
    o.push_str("40,ID,Unique ID,Name,Standard Rate,Cost Per Use\r\n41,40,49,1,42,44\r\n");
    for (k, r) in p.resources.iter().enumerate() {
        let _ = writeln!(o, "50,{},{},{},{}/h,{}\r", k + 1, r.uid, q(&r.name), r.rate, r.per_use);
    }
    o.push_str("60,Unique ID,ID,Name,Outline Level,Duration,Start,Finish,% Complete,Predecessors,Text1\r\n61,98,90,1,3,40,50,51,44,70,4\r\n");
    let id_of: std::collections::HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect();
    for (i, t) in p.tasks.iter().enumerate() {
        // "3", "3SS" or "3FS+1.5d": a lag needs the type; a percent lag becomes days of the predecessor.
        let preds: Vec<String> = t.preds.iter().filter_map(|l| {
            let k = id_of.get(&l.pred)?;
            let j = k - 1;
            let lag = if l.pct { p.tasks[j].dur * l.lag / 100 } else { l.lag };
            Some(match (l.kind, lag) {
                (LinkType::FS, 0) => k.to_string(),
                (kind, 0) => format!("{k}{kind:?}"),
                (kind, m) => format!("{k}{kind:?}{}{}d", if m > 0 { "+" } else { "" }, (m as f64 / p.day_min as f64 * 100.0).round() / 100.0),
            })
        }).collect();
        let _ = writeln!(o, "70,{},{},{},{},{}d,{},{},{},{},{}\r", t.uid, i + 1, q(&t.name), t.level + 1, (s.cals[s.ci[i]].between(s.es[i], s.ef[i]) as f64 / p.day_min as f64 * 100.0).round() / 100.0,
            mpx_date(s.es[i]), mpx_date((s.ef[i] - 1).max(s.es[i])), c.pct[i], q(&preds.join(",")), q(&t.code));
        // Record 75 names the resource by ID (field 1) and Unique ID (field 13); units are 1 = 100 %.
        for a in &t.res {
            let Some(k) = p.res_idx(a.res) else { continue };
            let _ = writeln!(o, "75,{},{}{}{}\r", k + 1, a.units, ",".repeat(11), a.res);
        }
    }
    o
}

fn pdt(t: i64) -> String { format!("{}T{:02}:{:02}:00", date_of(t), t.rem_euclid(DAY) / 60, t.rem_euclid(60)) }

/// PMXML (APIBusinessObjects): WBS, activities, relationships and resources.
pub fn pmxml(c: &Calc) -> String {
    let (p, s) = (c.p, &c.s);
    let mut o = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<APIBusinessObjects xmlns=\"http://xmlns.oracle.com/Primavera/P6/V19.12/API/BusinessObjects\">\n");
    for (k, cal) in p.calendars.iter().enumerate() {
        let _ = writeln!(o, "<Calendar><ObjectId>{}</ObjectId><Name>{}</Name><Type>Global</Type><HoursPerDay>{}</HoursPerDay><IsDefault>{}</IsDefault></Calendar>", k + 1, esc(&cal.name), p.day_min as f64 / 60.0, cal.name == p.calendar);
    }
    for r in &p.resources {
        let ty = match r.kind { ResKind::Work => "Labor", ResKind::Material => "Material", ResKind::Cost => "Nonlabor" };
        let _ = writeln!(o, "<Resource><ObjectId>{}</ObjectId><Id>{}</Id><Name>{}</Name><ResourceType>{ty}</ResourceType><PricePerUnit>{}</PricePerUnit></Resource>", r.uid, esc(if r.code.is_empty() { &r.name } else { &r.code }), esc(&r.name), r.rate);
    }
    let _ = writeln!(o, "<Project><ObjectId>1</ObjectId><Id>{}</Id><Name>{}</Name><PlannedStartDate>{}</PlannedStartDate><DataDate>{}</DataDate>",
        esc(if p.code.is_empty() { "DATUM" } else { &p.code }), esc(&p.name), pdt(s.start), pdt(c.status_at().min(s.finish.max(s.start))));
    let mut parent = vec![None::<usize>; p.tasks.len()];
    let mut stack: Vec<usize> = vec![];
    for (i, t) in p.tasks.iter().enumerate() {
        stack.retain(|&j| p.tasks[j].level < t.level);
        parent[i] = stack.last().copied();
        if s.summary[i] {
            let _ = writeln!(o, "<WBS><ObjectId>{}</ObjectId><Code>{}</Code><Name>{}</Name><ParentObjectId>{}</ParentObjectId><ProjectObjectId>1</ProjectObjectId></WBS>",
                100000 + t.uid, esc(&c.wbs[i]), esc(&t.name), parent[i].map_or(String::new(), |j| (100000 + p.tasks[j].uid).to_string()));
            stack.push(i);
        }
    }
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.summary[i]) {
        let ty = match t.act_type { ActType::Loe => "Level of Effort", ActType::StartMs => "Start Milestone", ActType::FinishMs => "Finish Milestone", ActType::ResourceDep => "Resource Dependent", ActType::WbsSummary => "WBS Summary", ActType::Task if t.dur == 0 => "Finish Milestone", ActType::Task => "Task Dependent" };
        let status = if c.pct[i] == 100 { "Completed" } else if c.pct[i] > 0 { "In Progress" } else { "Not Started" };
        let _ = writeln!(o, "<Activity><ObjectId>{}</ObjectId><Id>{}</Id><Name>{}</Name><Type>{ty}</Type><Status>{status}</Status><WBSObjectId>{}</WBSObjectId><PlannedDuration>{}</PlannedDuration>\
<RemainingDuration>{}</RemainingDuration><StartDate>{}</StartDate><FinishDate>{}</FinishDate><PhysicalPercentComplete>{}</PhysicalPercentComplete><PercentCompleteType>Physical</PercentCompleteType>\
<CalendarObjectId>{}</CalendarObjectId></Activity>",
            t.uid, esc(if t.code.is_empty() { &t.name } else { &t.code }), esc(&t.name), parent[i].map_or(String::new(), |j| (100000 + p.tasks[j].uid).to_string()),
            t.dur as f64 / 60.0, t.dur as f64 * (100 - c.pct[i] as i64) as f64 / 6000.0, pdt(s.es[i]), pdt(s.ef[i]), c.pct[i], s.ci[i].min(p.calendars.len() - 1) + 1);
    }
    // As in XER: a link to or from a summary applies to every activity under it; a percent lag becomes hours.
    let under = |i: usize| -> Vec<usize> { if s.summary[i] { p.subtree(i).filter(|&j| !s.summary[j]).collect() } else { vec![i] } };
    let mut n = 0;
    let idx: std::collections::HashMap<u32, usize> = p.tasks.iter().enumerate().rev().map(|(i, t)| (t.uid, i)).collect();
    for (i, t) in p.tasks.iter().enumerate() {
        for l in &t.preds {
            let Some(&k) = idx.get(&l.pred) else { continue };
            let lag = if l.pct { p.tasks[k].dur * l.lag / 100 } else { l.lag };
            let kind = match l.kind { LinkType::FS => "Finish to Start", LinkType::SS => "Start to Start", LinkType::FF => "Finish to Finish", LinkType::SF => "Start to Finish" };
            for b in under(i) { for a in under(k) {
                n += 1;
                let _ = writeln!(o, "<Relationship><ObjectId>{n}</ObjectId><PredecessorActivityObjectId>{}</PredecessorActivityObjectId><SuccessorActivityObjectId>{}</SuccessorActivityObjectId><Type>{kind}</Type><Lag>{}</Lag></Relationship>", p.tasks[a].uid, p.tasks[b].uid, lag as f64 / 60.0);
            } }
        }
    }
    let leaf: std::collections::HashSet<u32> = (0..p.tasks.len()).filter(|&i| !s.summary[i]).map(|i| p.tasks[i].uid).collect();
    let mut n = 0;
    for t in p.tasks.iter().filter(|t| leaf.contains(&t.uid)) {
        for a in &t.res {
            n += 1;
            let _ = writeln!(o, "<ResourceAssignment><ObjectId>{n}</ObjectId><ActivityObjectId>{}</ActivityObjectId><ResourceObjectId>{}</ResourceObjectId><PlannedUnitsPerTime>{}</PlannedUnitsPerTime></ResourceAssignment>", t.uid, a.res, a.units);
        }
    }
    o.push_str("</Project>\n</APIBusinessObjects>\n");
    o
}
