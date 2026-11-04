//! XER: tab-separated tables (%T table, %F fields, %R rows). Summary tasks become WBS
//! nodes, other tasks become activities.
use crate::cal::{at, date_of, DAY};
use crate::model::*;
use crate::sched::Sched;
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

fn dt(t: i64) -> String {
    format!("{} {:02}:{:02}", date_of(t), t.rem_euclid(DAY) / 60, t.rem_euclid(60))
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// Elapsed durations run on this round-the-clock calendar in the file (XER has no elapsed durations).
const ELAPSED: &str = "Elapsed (24 hours)";

const CS: [(Constraint, &str); 7] = [
    (Constraint::MSO, "CS_MSO"), (Constraint::MFO, "CS_MEO"), (Constraint::SNET, "CS_MSOA"), (Constraint::SNLT, "CS_MSOB"),
    (Constraint::FNET, "CS_MEOA"), (Constraint::FNLT, "CS_MEOB"), (Constraint::ALAP, "CS_ALAP"),
];

fn excel_day(d: NaiveDate) -> i64 {
    (d - NaiveDate::from_ymd_opt(1899, 12, 30).unwrap()).num_days()
}

/// XER calendar data: nested "(0||name(args)(children))" nodes.
fn clndr_data(c: &Calendar) -> String {
    let hours = |h: &Hours| h.iter().enumerate().map(|(k, (s, e))| format!("(0||{k}(s|{:02}:{:02}|f|{:02}:{:02})())", s / 60, s % 60, (e / 60) % 24, e % 60)).collect::<String>();
    let days: String = (0..7).map(|k| format!("(0||{}()({}))", (k + 1) % 7 + 1, hours(&c.week[k]))).collect();
    let mut exc = String::new();
    let mut n = 0;
    for e in &c.exceptions {
        let mut d = e.from;
        while d <= e.to && n < 5000 {
            let _ = write!(exc, "(0||{n}(d|{})({}))", excel_day(d), hours(&e.hours));
            d = d.succ_opt().unwrap();
            n += 1;
        }
    }
    format!("(0||CalendarData()((0||DaysOfWeek()({days}))(0||VIEW(ShowTotal|Y)())(0||Exceptions()({exc}))))")
}

type Row = HashMap<String, String>;

struct PNode { name: String, args: String, kids: Vec<PNode> }

/// One node of the XER calendar text; `depth` stops damaged text from nesting without end.
fn pnode(s: &[u8], i: &mut usize, depth: u8) -> Option<PNode> {
    let ws = |i: &mut usize| while *i < s.len() && (s[*i] as char).is_whitespace() || (*i < s.len() && s[*i] == 0x7f) { *i += 1 };
    ws(i);
    if s.get(*i) != Some(&b'(') || depth > 32 {
        return None;
    }
    *i += 1;
    while *i < s.len() && s[*i] != b'|' { *i += 1 }
    *i = (*i + 2).min(s.len()); // "||"
    let st = *i;
    while *i < s.len() && s[*i] != b'(' { *i += 1 }
    let name = String::from_utf8_lossy(&s[st..*i]).trim().to_string();
    *i = (*i + 1).min(s.len());
    let st = *i;
    while *i < s.len() && s[*i] != b')' { *i += 1 }
    let args = String::from_utf8_lossy(&s[st..*i]).to_string();
    *i += 1;
    ws(i);
    let mut kids = vec![];
    if s.get(*i) == Some(&b'(') {
        *i += 1;
        while let Some(k) = pnode(s, i, depth + 1) {
            kids.push(k);
        }
        ws(i);
        *i += 1; // ")" closing children
    }
    ws(i);
    *i += 1; // ")" closing node
    Some(PNode { name, args, kids })
}

fn read_cal(name: &str, data: &str, day_hours: f64) -> Calendar {
    let mut c = Calendar::standard(name);
    let s = data.as_bytes();
    let Some(root) = pnode(s, &mut 0, 0) else { return c };
    let hours = |n: &PNode| -> Hours {
        n.kids.iter().filter_map(|h| {
            let p: Vec<&str> = h.args.split('|').collect();
            let m = |x: &str| -> Option<u16> { Some(x.get(..2)?.parse::<u16>().ok()? * 60 + x.get(3..5)?.parse::<u16>().ok()?) };
            let v = |k: &str| p.iter().position(|x| *x == k).and_then(|i| p.get(i + 1)).and_then(|x| m(x));
            let (a, b) = (v("s")?, v("f")?);
            Some((a, if b == 0 { 1440 } else { b }))
        }).collect()
    };
    for sec in &root.kids {
        if sec.name == "DaysOfWeek" {
            for d in &sec.kids {
                if let Ok(k) = d.name.parse::<usize>() {
                    if (1..=7).contains(&k) {
                        c.week[(k + 5) % 7] = hours(d);
                    }
                }
            }
        } else if sec.name == "Exceptions" {
            for e in &sec.kids {
                if let Some(d) = e.args.split('|').nth(1).and_then(|x| x.parse::<u64>().ok()).and_then(|n| NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_days(chrono::Days::new(n))) {
                    c.exceptions.push(Exception { name: "Exception".into(), from: d, to: d, hours: hours(e) });
                }
            }
        }
    }
    if c.week.iter().all(Vec::is_empty) {
        let h = vec![(480u16, 480 + (day_hours * 60.0) as u16)];
        c.week = std::array::from_fn(|k| if k < 5 { h.clone() } else { vec![] });
    }
    c
}

pub fn write(p: &Project, s: &Sched, today: NaiveDate) -> String {
    let mut o = format!("ERMHDR\t19.12\t{today}\tProject\tadmin\tDatum Plan\tdbxDatabaseNoName\tProject Management\tUSD\n");
    let mut table = |name: &str, fields: &str, rows: Vec<String>| {
        let _ = writeln!(o, "%T\t{name}\n%F\t{}", fields.replace(' ', "\t"));
        rows.iter().for_each(|r| { let _ = writeln!(o, "%R\t{r}"); });
    };
    let short: String = p.name.chars().filter(|c| c.is_alphanumeric()).take(20).collect::<String>().to_uppercase();
    let short = if short.is_empty() { "DATUM".into() } else { short };
    let cal_id = |n: Option<&str>| p.calendars.iter().position(|c| c.name == n.unwrap_or(&p.calendar)).unwrap_or(0) + 1;
    let hd = p.day_min as f64 / 60.0;
    // A person with leave gets a resource calendar: their base calendar plus the leave.
    let own = |r: &crate::model::Resource| (!r.exceptions.is_empty()).then(|| {
        let mut c = p.calendars.iter().find(|c| Some(&c.name) == r.cal.as_ref()).unwrap_or(&p.calendars[cal_id(None) - 1]).clone();
        c.exceptions.extend(r.exceptions.iter().cloned());
        c.name = r.name.clone();
        c
    });
    let rcal: Vec<usize> = p.resources.iter().enumerate().map(|(k, r)| if own(r).is_some() { 1000 + k } else { cal_id(r.cal.as_deref()) }).collect();
    let mut cal_rows: Vec<String> = p.calendars.iter().enumerate().map(|(k, c)| format!("{}\t{}\t{}\t1\t\t{today} 00:00\tCA_Project\t{hd}\t{}\t{}\t{}\tN\t{}",
        k + 1, if c.name == p.calendar { "Y" } else { "N" }, clean(&c.name), hd * 5.0, hd * 20.0, hd * 250.0, clndr_data(c))).collect();
    for (k, r) in p.resources.iter().enumerate() {
        if let Some(c) = own(r) {
            cal_rows.push(format!("{}\tN\t{}\t\t{}\t{today} 00:00\tCA_Rsrc\t{hd}\t{}\t{}\t{}\tY\t{}", 1000 + k, clean(&c.name), cal_id(r.cal.as_deref()), hd * 5.0, hd * 20.0, hd * 250.0, clndr_data(&c)));
        }
    }
    let el = p.calendars.len() + 1;
    if p.tasks.iter().any(|t| t.elapsed) {
        cal_rows.push(format!("{el}\tN\t{ELAPSED}\t1\t\t{today} 00:00\tCA_Project\t24\t168\t720\t8760\tN\t{}", clndr_data(&Calendar::full(ELAPSED))));
    }
    table("CALENDAR", "clndr_id default_flag clndr_name proj_id base_clndr_id last_chng_date clndr_type day_hr_cnt week_hr_cnt month_hr_cnt year_hr_cnt rsrc_private clndr_data", cal_rows);
    let data = p.status.map(|d| dt(at(d) + DAY)).unwrap_or_else(|| dt(s.start));
    table("PROJECT", "proj_id proj_short_name clndr_id plan_start_date last_recalc_date scd_end_date def_complete_pct_type",
        vec![format!("1\t{short}\t{}\t{}\t{data}\t{}\tCP_Phys", cal_id(None), dt(s.start), dt(s.finish))]);
    // WBS: the project node, then one node per summary task.
    let mut wbs_rows = vec![format!("1\t1\t0\tY\tWS_Open\t{short}\t{}\t", clean(&p.name))];
    let mut wbs_of = vec![1usize; p.tasks.len()];
    let mut stack: Vec<(u8, usize)> = vec![];
    for (i, t) in p.tasks.iter().enumerate() {
        stack.retain(|&(l, _)| l < t.level);
        let parent = stack.last().map_or(1, |&(_, w)| w);
        wbs_of[i] = parent;
        if s.summary[i] {
            let id = 100 + i;
            let code = if t.code.is_empty() { format!("W{}", i + 1) } else { clean(&t.code) };
            wbs_rows.push(format!("{id}\t1\t{}\tN\tWS_Open\t{code}\t{}\t{parent}", i + 1, clean(&t.name)));
            stack.push((t.level, id));
        }
    }
    table("PROJWBS", "wbs_id proj_id seq_num proj_node_flag status_code wbs_short_name wbs_name parent_wbs_id", wbs_rows);
    let acts: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i]).collect();
    let held = super::ext_held(p);
    table("TASK", "task_id proj_id wbs_id clndr_id phys_complete_pct complete_pct_type task_type duration_type status_code task_code task_name target_drtn_hr_cnt remain_drtn_hr_cnt act_start_date act_end_date early_start_date early_end_date target_start_date target_end_date cstr_date cstr_type cstr_date2 cstr_type2",
        acts.iter().map(|&i| {
            let t = &p.tasks[i];
            let rsrc = t.act_type == ActType::ResourceDep || (p.opts.res_cals && !t.ignore_rc && t.res.iter().any(|a| p.res_idx(a.res).is_some_and(|k| p.resources[k].kind == ResKind::Work)));
            let (dur, cons, cdate) = super::held(p, s, i, held[i]);
            // A milestone at the end of a working period is a finish milestone (a start milestone would move to the next morning).
            let kind = match t.act_type { ActType::Loe => "TT_LOE", ActType::WbsSummary => "TT_WBS", ActType::StartMs => "TT_Mile", ActType::FinishMs => "TT_FinMile",
                _ if dur == 0 => if s.cals[s.ci[i]].next_start(s.es[i]) == s.es[i] { "TT_Mile" } else { "TT_FinMile" }, _ if rsrc => "TT_Rsrc", _ => "TT_Task" };
            let status = if t.act_finish.is_some() { "TK_Complete" } else if t.act_start.is_some() { "TK_Active" } else { "TK_NotStart" };
            let (cd, ct) = match (cdate, CS.iter().find(|(c, _)| *c == cons)) {
                (_, Some((Constraint::ALAP, n))) => (String::new(), n.to_string()),
                (Some(d), Some((_, n))) => (format!("{d} 00:00"), n.to_string()),
                _ => (String::new(), String::new()),
            };
            let code = if t.code.is_empty() { format!("A{}", (i + 1) * 10) } else { clean(&t.code) };
            // A physical-% task carries its physical %; the duration % stays in the remaining duration.
            let (pc, pt) = match t.pct_type { PctType::Physical => (t.phys, "CP_Phys"), PctType::Units => (t.pct, "CP_Units"), PctType::Duration => (t.pct, "CP_Drtn") };
            // A deadline goes as a secondary "finish on or before" constraint.
            let (cd2, ct2) = t.deadline.map_or((String::new(), ""), |d| (format!("{d} 23:59"), "CS_MEOB"));
            format!("{}\t1\t{}\t{}\t{pc}\t{pt}\t{kind}\tDT_FixedDUR2\t{status}\t{code}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{cd}\t{ct}\t{cd2}\t{ct2}",
                t.uid, wbs_of[i], if t.elapsed { el } else { cal_id(t.cal.as_deref()) }, clean(&t.name), dur as f64 / 60.0, dur as f64 * (100 - t.pct as i64) as f64 / 6000.0,
                t.act_start.map(|d| format!("{d} 08:00")).unwrap_or_default(), t.act_finish.map(|d| format!("{d} 17:00")).unwrap_or_default(),
                dt(s.es[i]), dt(s.ef[i]), dt(s.es[i]), dt(s.ef[i]))
        }).collect());
    // A link to or from a summary applies to every activity under it; a percent lag becomes hours.
    let under = |i: usize| -> Vec<usize> { if s.summary[i] { p.subtree(i).filter(|&j| !s.summary[j]).collect() } else { vec![i] } };
    let mut links = vec![];
    let idx: HashMap<u32, usize> = p.tasks.iter().enumerate().rev().map(|(i, t)| (t.uid, i)).collect();
    for (i, t) in p.tasks.iter().enumerate() {
        for l in &t.preds {
            let Some(&k) = idx.get(&l.pred) else { continue };
            let lag = if l.pct { p.tasks[k].dur * l.lag / 100 } else { l.lag };
            for b in under(i) { for a in under(k) { links.push(format!("{}\t{}\t{}\t1\t1\tPR_{:?}\t{}", links.len() + 1, p.tasks[b].uid, p.tasks[a].uid, l.kind, lag as f64 / 60.0)) } }
        }
    }
    table("TASKPRED", "task_pred_id task_id pred_task_id proj_id pred_proj_id pred_type lag_hr_cnt", links);
    table("RSRC", "rsrc_id rsrc_name rsrc_short_name rsrc_type clndr_id", p.resources.iter().enumerate().map(|(k, r)| {
        let ty = match r.kind { ResKind::Work => "RT_Labor", ResKind::Material => "RT_Mat", ResKind::Cost => "RT_Equip" };
        let short: String = r.name.chars().filter(|c| c.is_alphanumeric()).take(20).collect();
        format!("{}\t{}\t{}\t{ty}\t{}", r.uid, clean(&r.name), short.to_uppercase(), rcal[k])
    }).collect());
    table("RSRCRATE", "rsrc_rate_id rsrc_id cost_per_qty", p.resources.iter().enumerate().map(|(k, r)| format!("{}\t{}\t{}", k + 1, r.uid, r.rate)).collect());
    let mut n = 0;
    table("TASKRSRC", "taskrsrc_id task_id proj_id rsrc_id target_qty remain_qty target_qty_per_hr target_cost", acts.iter().flat_map(|&i| p.tasks[i].res.iter().map(move |a| (i, a))).map(|(i, a)| {
        n += 1;
        let t = &p.tasks[i];
        // A cost resource has no quantity, only an amount.
        let kind = p.res_idx(a.res).map(|k| p.resources[k].kind);
        if kind == Some(ResKind::Cost) { return format!("{n}\t{}\t1\t{}\t0\t0\t0\t{}", t.uid, a.res, a.units) }
        let q = if kind == Some(ResKind::Work) { t.dur as f64 / 60.0 * a.units } else { a.units };
        format!("{n}\t{}\t1\t{}\t{q}\t{}\t{}\t", t.uid, a.res, q * (100 - t.pct as i64) as f64 / 100.0, a.units)
    }).collect());
    o.push_str("%E\n");
    o
}

pub fn read(text: &str) -> Result<Project, String> { read_notes(text).map(|x| x.0) }

/// Reads an XER file and notes what was changed or left out on the way in.
pub fn read_notes(text: &str) -> Result<(Project, Vec<String>), String> {
    let mut notes = vec![];
    let mut tables: HashMap<String, Vec<HashMap<String, String>>> = HashMap::new();
    let (mut cur, mut fields): (String, Vec<String>) = (String::new(), vec![]);
    for line in text.lines() {
        let mut parts = line.split('\t');
        match parts.next() {
            Some("%T") => cur = parts.next().unwrap_or("").trim().to_string(),
            Some("%F") => fields = parts.map(|f| f.trim().to_string()).collect(),
            Some("%R") => {
                let row = fields.iter().cloned().zip(parts.map(String::from)).collect();
                tables.entry(cur.clone()).or_default().push(row);
            }
            _ => {}
        }
    }
    let rows = |t: &str| tables.get(t).map(Vec::as_slice).unwrap_or(&[]);
    let proj = rows("PROJECT").first().ok_or("This XER file has no project.")?;
    let pid = proj.get("proj_id").cloned().unwrap_or_default();
    let mine = |r: &&HashMap<String, String>| r.get("proj_id").is_none_or(|x| *x == pid);
    let g = |r: &HashMap<String, String>, k: &str| r.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
    let date = |s: String| NaiveDate::parse_from_str(s.get(..10).unwrap_or(""), "%Y-%m-%d").ok();
    let start = date(g(proj, "plan_start_date")).unwrap_or_else(|| chrono::Local::now().date_naive());
    let name = [g(proj, "proj_short_name")].into_iter().find(|s| !s.is_empty()).unwrap_or_else(|| "Imported project".into());
    let mut p = Project::new(&name, start);
    p.status = date(g(proj, "last_recalc_date")).and_then(|d| d.pred_opt()).filter(|d| *d >= start);
    let mut cal_names = HashMap::new();
    let cals: Vec<&HashMap<String, String>> = rows("CALENDAR").iter().collect();
    let pcal = g(proj, "clndr_id");
    let hd = cals.iter().find(|c| g(c, "clndr_id") == pcal).and_then(|c| g(c, "day_hr_cnt").parse::<f64>().ok()).unwrap_or(8.0);
    p.day_min = (hd * 60.0).round() as i64;
    if !cals.is_empty() {
        p.calendars = vec![];
        for c in &cals {
            let mut name = g(c, "clndr_name");
            if name.is_empty() || p.calendars.iter().any(|x| x.name == name) {
                name = format!("{name} {}", g(c, "clndr_id")).trim().to_string();
            }
            cal_names.insert(g(c, "clndr_id"), name.clone());
            p.calendars.push(read_cal(&name, &g(c, "clndr_data"), g(c, "day_hr_cnt").parse().unwrap_or(8.0)));
        }
        p.calendar = cal_names.get(&pcal).cloned().or_else(|| cals.iter().find(|c| g(c, "default_flag") == "Y").and_then(|c| cal_names.get(&g(c, "clndr_id")).cloned())).unwrap_or_else(|| p.calendars[0].name.clone());
    }
    // WBS tree, ordered by seq_num.
    let wbs: Vec<&HashMap<String, String>> = rows("PROJWBS").iter().filter(mine).collect();
    let root = wbs.iter().find(|w| g(w, "proj_node_flag") == "Y").map(|w| g(w, "wbs_id"));
    let mut kids: HashMap<String, Vec<&HashMap<String, String>>> = HashMap::new();
    for w in &wbs {
        if Some(g(w, "wbs_id")) != root {
            kids.entry(g(w, "parent_wbs_id")).or_default().push(w);
        }
    }
    kids.values_mut().for_each(|v| v.sort_by_key(|w| g(w, "seq_num").parse::<i64>().unwrap_or(0)));
    let mut acts: HashMap<String, Vec<&HashMap<String, String>>> = HashMap::new();
    for t in rows("TASK").iter().filter(mine) {
        acts.entry(g(t, "wbs_id")).or_default().push(t);
    }
    acts.values_mut().for_each(|v| v.sort_by_key(|a| (g(a, "early_start_date"), g(a, "task_code"))));
    // Under each WBS node: its activities, then its child nodes. A file Datum Plan wrote keeps the
    // plan's rows: nodes sit at their row (seq_num) and activities fill the other rows in file order.
    let mut place: HashMap<*const Row, usize> = HashMap::new();
    if text.lines().next().is_some_and(|l| l.split('\t').nth(5) == Some("Datum Plan")) {
        let taken: HashSet<usize> = wbs.iter().filter_map(|w| g(w, "seq_num").parse::<usize>().ok()).collect();
        let mut free = (1..).filter(|k| !taken.contains(k));
        for t in rows("TASK").iter().filter(mine) { place.insert(t as *const _, free.next().unwrap_or(0)); }
        for w in &wbs { place.insert(*w as *const _, g(w, "seq_num").parse().unwrap_or(0)); }
    }
    let cons = |s: &str| CS.iter().find(|(_, n)| *n == s).map(|(c, _)| *c).unwrap_or_default();
    let mut uid = 1_000_000u32;
    // `seen` walks each WBS node once, so a damaged file whose WBS loops back on itself still opens.
    #[allow(clippy::too_many_arguments)]
    fn walk<'a>(id: &'a str, level: u8, seen: &mut HashSet<&'a str>, p: &mut Project, kids: &'a HashMap<String, Vec<&'a Row>>, acts: &'a HashMap<String, Vec<&'a Row>>,
        place: &HashMap<*const Row, usize>, mk: &mut dyn FnMut(&Row, u8) -> Task, sum: &mut dyn FnMut(&Row, u8) -> Task) {
        if !seen.insert(id) {
            return;
        }
        let key = |r: &Row| place.get(&(r as *const _)).copied().unwrap_or(0);
        let mut items: Vec<(usize, bool, &Row)> = acts.get(id).into_iter().flatten().map(|t| (key(t), false, *t)).collect();
        items.extend(kids.get(id).into_iter().flatten().map(|w| (key(w), true, *w)));
        items.sort_by_key(|x| (x.0, x.1));
        for (_, node, r) in items {
            if !node {
                let task = mk(r, level);
                p.tasks.push(task);
                continue;
            }
            let at = p.tasks.len();
            let s = sum(r, level);
            p.tasks.push(s);
            walk(r.get("wbs_id").map_or("", |s| s.trim()), level.saturating_add(1), seen, p, kids, acts, place, mk, sum);
            if p.tasks.len() == at + 1 {
                p.tasks.pop(); // an empty WBS node
            }
        }
    }
    let mut mk = |t: &HashMap<String, String>, level: u8| {
        let ty = g(t, "task_type");
        let dur = if ty.contains("Mile") { 0 } else { (g(t, "target_drtn_hr_cnt").parse::<f64>().unwrap_or(0.0) * 60.0).round() as i64 };
        let ct = cons(&g(t, "cstr_type"));
        let phys = g(t, "complete_pct_type") == "CP_Phys";
        let pc = g(t, "phys_complete_pct").parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0).round() as u8;
        // With physical %, the duration % comes from the remaining duration when the file has one.
        let (target, remain) = (g(t, "target_drtn_hr_cnt").parse::<f64>().unwrap_or(0.0), g(t, "remain_drtn_hr_cnt").parse::<f64>().ok());
        let pct = match remain { Some(r) if phys && target > 0.0 => ((1.0 - r / target) * 100.0).clamp(0.0, 100.0).round() as u8, _ => pc };
        let act_type = match ty.as_str() { "TT_LOE" => ActType::Loe, "TT_WBS" => ActType::WbsSummary, "TT_Rsrc" => ActType::ResourceDep, "TT_Mile" => ActType::StartMs, "TT_FinMile" => ActType::FinishMs, _ => ActType::Task };
        Task {
            uid: g(t, "task_id").parse().unwrap_or(0), name: g(t, "task_name"), level, dur, code: g(t, "task_code"), act_type, ignore_rc: ty == "TT_Task",
            pct, phys: if phys { pc } else { 0 }, pct_type: if phys { PctType::Physical } else { PctType::Duration },
            constraint: ct, cdate: if matches!(ct, Constraint::ASAP | Constraint::ALAP) { None } else { date(g(t, "cstr_date")) },
            cal: cal_names.get(&g(t, "clndr_id")).cloned(),
            act_start: date(g(t, "act_start_date")), act_finish: date(g(t, "act_end_date")),
            deadline: if g(t, "cstr_type2") == "CS_MEOB" { date(g(t, "cstr_date2")) } else { None },
            ..Default::default()
        }
    };
    let mut sum = |w: &HashMap<String, String>, level: u8| {
        uid += 1;
        Task { uid, name: g(w, "wbs_name"), code: g(w, "wbs_short_name"), level, ..Default::default() }
    };
    walk(root.as_deref().unwrap_or(""), 0, &mut HashSet::new(), &mut p, &kids, &acts, &place, &mut mk, &mut sum);
    // Activities on the project calendar follow it (a later change of project calendar moves them too).
    let pc = p.calendar.clone();
    p.tasks.iter_mut().filter(|t| t.cal.as_ref() == Some(&pc)).for_each(|t| t.cal = None);
    // A task on the round-the-clock calendar Datum Plan writes for them is elapsed again.
    if p.calendars.iter().any(|c| c.name == ELAPSED && *c == Calendar::full(ELAPSED)) {
        p.tasks.iter_mut().filter(|t| t.cal.as_deref() == Some(ELAPSED)).for_each(|t| (t.elapsed, t.cal, t.unit) = (true, None, Unit::D));
        p.calendars.retain(|c| c.name != ELAPSED || p.calendar == ELAPSED);
    }
    // Activities outside the WBS tree go at the end.
    let have: std::collections::HashSet<String> = p.tasks.iter().map(|x| x.uid.to_string()).collect();
    let rest: Vec<Task> = rows("TASK").iter().filter(mine).filter(|t| !have.contains(&g(t, "task_id"))).map(|t| mk(t, 0)).collect();
    if !rest.is_empty() { notes.push(format!("{} activities are outside the WBS; they are at the end of the plan.", rest.len())) }
    p.tasks.extend(rest);
    let second = rows("TASK").iter().filter(mine).filter(|t| !matches!(g(t, "cstr_type2").as_str(), "" | "CS_MEOB")).count();
    if second > 0 { notes.push(format!("{second} activities have a secondary constraint other than finish on or before (kept as a deadline), which Datum Plan does not use yet; only the primary constraint was kept.")) }
    let mut dropped = 0;
    let idx: HashMap<u32, usize> = p.tasks.iter().enumerate().rev().map(|(i, t)| (t.uid, i)).collect();
    for l in rows("TASKPRED") {
        let (s, q): (u32, u32) = (g(l, "task_id").parse().unwrap_or(0), g(l, "pred_task_id").parse().unwrap_or(0));
        let kind = match g(l, "pred_type").as_str() { "PR_SS" => LinkType::SS, "PR_FF" => LinkType::FF, "PR_SF" => LinkType::SF, _ => LinkType::FS };
        let lag = (g(l, "lag_hr_cnt").parse::<f64>().unwrap_or(0.0) * 60.0).round() as i64;
        match idx.get(&q).and(idx.get(&s)) {
            Some(&i) => p.tasks[i].preds.push(Link { pred: q, kind, lag, pct: false }),
            None => dropped += 1,
        }
    }
    if dropped > 0 { notes.push(format!("{dropped} relationships point to activities that are not in this project (another project or a deleted activity); they were left out.")) }
    for r in rows("RSRC") {
        let kind = if g(r, "rsrc_type") == "RT_Mat" { ResKind::Material } else { ResKind::Work };
        let cal = cal_names.get(&g(r, "clndr_id")).cloned().filter(|c| *c != p.calendar);
        let uid = g(r, "rsrc_id");
        let rate = rows("RSRCRATE").iter().find(|x| g(x, "rsrc_id") == uid).and_then(|x| g(x, "cost_per_qty").parse().ok()).unwrap_or(0.0);
        p.resources.push(Resource { uid: uid.parse().unwrap_or(0), name: g(r, "rsrc_name"), kind, cal, rate, ..Default::default() });
    }
    for a in rows("TASKRSRC").iter().filter(mine) {
        let (tu, ru): (u32, u32) = (g(a, "task_id").parse().unwrap_or(0), g(a, "rsrc_id").parse().unwrap_or(0));
        let qty = g(a, "target_qty").parse::<f64>().unwrap_or(0.0);
        let per_hr = g(a, "target_qty_per_hr").parse::<f64>().ok();
        let cost = g(a, "target_cost").parse::<f64>().unwrap_or(0.0);
        // An assignment with an amount and no quantity is a cost resource.
        let money = qty == 0.0 && cost > 0.0;
        if let Some(r) = p.resources.iter_mut().find(|r| r.uid == ru && money) { r.kind = ResKind::Cost }
        let work = p.resources.iter().any(|r| r.uid == ru && r.kind == ResKind::Work);
        if let Some(t) = idx.get(&tu).map(|&i| &mut p.tasks[i]) {
            let units = if money { cost } else if work { per_hr.unwrap_or(if t.dur > 0 { qty * 60.0 / t.dur as f64 } else { 1.0 }) } else { qty };
            t.res.push(Assign { res: ru, units, table: 0 });
        }
    }
    p.normalize();
    Ok((p, notes))
}
