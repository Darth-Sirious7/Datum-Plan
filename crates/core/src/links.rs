//! Links between plans: a task's predecessors in other plan files. The scheduler holds a task by
//! each external predecessor's last known dates (see `sched`); `refresh` brings those dates up to
//! date through `find`, which the window answers from its open tabs or from the files on disk.
use crate::model::{ExtLink, Project};
use serde::Serialize;

/// What `find` reports for a task in another plan.
pub struct Found { pub plan: String, pub code: String, pub name: String, pub start: i64, pub finish: i64 }

/// One link as the Linked Plans panel shows it.
#[derive(Serialize)]
pub struct LinkState {
    pub uid: u32,
    pub task: String,
    /// Position in the task's `ext` list.
    pub k: usize,
    pub link: ExtLink,
    /// The other task's finish now (when the file could be read).
    pub now_start: Option<i64>,
    pub now_finish: Option<i64>,
    /// "ok", "changed" (the other plan moved since the last update) or the reason it was not found.
    pub state: String,
}

/// Every link of the plan with its state, without changing anything.
pub fn states(p: &Project, find: &mut impl FnMut(&ExtLink) -> Result<Found, String>) -> Vec<LinkState> {
    let mut out = vec![];
    for t in &p.tasks {
        for (k, x) in t.ext.iter().enumerate() {
            let (now, state) = match find(x) {
                Ok(f) if (f.start, f.finish) == (x.start, x.finish) => (Some((f.start, f.finish)), "ok".to_string()),
                Ok(f) => (Some((f.start, f.finish)), "changed".to_string()),
                Err(e) => (None, e),
            };
            out.push(LinkState { uid: t.uid, task: t.name.clone(), k, link: x.clone(), now_start: now.map(|n| n.0), now_finish: now.map(|n| n.1), state });
        }
    }
    out
}

/// Copies the other plans' current dates into every link; returns how many changed and what could not be found.
pub fn refresh(p: &mut Project, find: &mut impl FnMut(&ExtLink) -> Result<Found, String>) -> (usize, Vec<String>) {
    let (mut changed, mut missing) = (0, vec![]);
    for t in &mut p.tasks {
        for x in &mut t.ext {
            match find(x) {
                Ok(f) => {
                    if (f.start, f.finish) != (x.start, x.finish) || f.name != x.name || f.code != x.code || f.plan != x.plan {
                        changed += 1;
                    }
                    (x.plan, x.code, x.name, x.start, x.finish) = (f.plan, f.code, f.name, f.start, f.finish);
                }
                Err(e) => missing.push(format!("{}: {e}", t.name)),
            }
        }
    }
    (changed, missing)
}

/// A link as people read it: "Engineering › E1180 Final documentation issued (FS+5d)", or (FS+7ed) elapsed.
pub fn label(x: &ExtLink, day_min: i64) -> String {
    format!("{} › {}{} ({})", x.plan, if x.code.is_empty() { String::new() } else { format!("{} ", x.code) }, x.name, kind_lag(x, day_min))
}

/// The type and lag as the task sheet writes them: "FS", "SS+2d", "FS+7ed".
pub fn kind_lag(x: &ExtLink, day_min: i64) -> String { format!("{:?}{}", x.kind, crate::parse::fmt_lag_el(x.lag, day_min, x.elapsed)) }

/// The file name a link points at (for finding it next to this plan when the folder moved).
pub fn file_name(file: &str) -> &str {
    file.rsplit(['/', '\\']).next().unwrap_or(file)
}

/// A task as seen from another plan: uid, ID, name, start and finish.
type Seen = (u32, String, String, i64, i64);

/// Brings the links of several plans (file path, plan) up to date from each other, pass after
/// pass until no date moves. Plans may link both ways (main → subplan → main); returns the passes
/// needed and the links that could not be followed, or an error when the dates never settle
/// (links that loop through the plans). A link finds its plan by path, else by file name.
pub fn update_all(plans: &mut [(String, Project)]) -> Result<(usize, Vec<String>), String> {
    let key = |f: &str| f.replace('\\', "/").to_lowercase();
    // A pass moves a date across one link between plans; a chain without a loop crosses each link
    // at most once, so more passes than links means the dates loop.
    let links: usize = plans.iter().map(|(_, p)| p.tasks.iter().map(|t| t.ext.len()).sum::<usize>()).sum();
    for pass in 1..=links + 2 {
        let found: Vec<(String, String, Vec<Seen>)> = plans.iter().map(|(f, p)| {
            let s = crate::sched::schedule(p).ok();
            (key(f), p.name.clone(), p.tasks.iter().enumerate().map(|(i, t)| (t.uid, t.code.clone(), t.name.clone(), s.as_ref().map_or(0, |s| s.es[i]), s.as_ref().map_or(0, |s| s.ef[i]))).collect())
        }).collect();
        let (mut moved, mut missing) = (0, vec![]);
        for (_, p) in plans.iter_mut() {
            let (m, miss) = refresh(p, &mut |x| {
                let (_, plan, tasks) = found.iter().find(|(f, _, _)| *f == key(&x.file))
                    .or_else(|| found.iter().find(|(f, _, _)| file_name(f) == file_name(&x.file).to_lowercase())).ok_or("the plan is not open")?;
                let (_, code, name, s, f) = tasks.iter().find(|t| t.0 == x.uid).ok_or("the task is no longer in that plan")?;
                Ok(Found { plan: plan.clone(), code: code.clone(), name: name.clone(), start: *s, finish: *f })
            });
            moved += m;
            missing.extend(miss);
        }
        if moved == 0 { return Ok((pass, missing)) }
    }
    Err("The dates keep moving: some links between the plans form a loop. Check the links in the Linked Plans panel.".into())
}
