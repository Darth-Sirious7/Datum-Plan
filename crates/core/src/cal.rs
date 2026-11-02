//! Working-time arithmetic on one calendar. Instants are minutes since 1970-01-01 00:00.
use crate::model::{Calendar, STD_HOURS};
use chrono::{Datelike, Days, NaiveDate};
use std::collections::HashMap;

pub const DAY: i64 = 1440;

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default()
}

/// The calendar's horizon: working-time arithmetic stops (saturates) at these days, so damaged
/// values (a duration of a million years) cannot walk off the end of the date range.
pub fn lo() -> NaiveDate { NaiveDate::from_ymd_opt(1800, 1, 1).unwrap_or_default() }
pub fn hi() -> NaiveDate { NaiveDate::from_ymd_opt(2600, 12, 31).unwrap_or_default() }
fn next(d: NaiveDate) -> Option<NaiveDate> { d.succ_opt().filter(|x| *x <= hi()) }
fn prev(d: NaiveDate) -> Option<NaiveDate> { d.pred_opt().filter(|x| *x >= lo()) }

/// Midnight at the start of `d`.
pub fn at(d: NaiveDate) -> i64 {
    (d - epoch()).num_days() * DAY
}

pub fn date_of(t: i64) -> NaiveDate {
    let n = t.div_euclid(DAY);
    let d = if n >= 0 { epoch().checked_add_days(Days::new(n as u64)) } else { epoch().checked_sub_days(Days::new(n.unsigned_abs())) };
    d.unwrap_or(if n >= 0 { NaiveDate::MAX } else { NaiveDate::MIN })
}

type Iv = Vec<(i64, i64)>;

pub struct WorkCal {
    week: [Iv; 7],
    exc: HashMap<NaiveDate, Iv>,
}

fn iv(h: &[(u16, u16)]) -> Iv {
    let mut v: Iv = h.iter().filter(|(s, e)| e > s).map(|&(s, e)| (s as i64, (e as i64).min(DAY))).collect();
    v.sort_unstable();
    v
}

impl WorkCal {
    pub fn new(c: &Calendar) -> Self {
        let mut week: [Iv; 7] = std::array::from_fn(|i| iv(&c.week[i]));
        if week.iter().all(Vec::is_empty) {
            week = std::array::from_fn(|i| if i < 5 { iv(&STD_HOURS) } else { vec![] });
        }
        let mut exc = HashMap::new();
        for e in &c.exceptions {
            let mut d = e.from;
            let mut n = 0;
            while d <= e.to && n < 3700 {
                exc.insert(d, iv(&e.hours));
                let Some(x) = d.succ_opt() else { break };
                (d, n) = (x, n + 1);
            }
        }
        Self { week, exc }
    }

    /// Working only when both calendars work (a task calendar and a person's calendar). If the two
    /// never work at the same time, `self` is kept so dates can still be worked out.
    pub fn intersect(&self, o: &WorkCal) -> WorkCal {
        let both = |a: &[(i64, i64)], b: &[(i64, i64)]| -> Iv {
            let mut v = vec![];
            for &(s1, e1) in a { for &(s2, e2) in b { let (s, e) = (s1.max(s2), e1.min(e2)); if e > s { v.push((s, e)) } } }
            v.sort_unstable();
            v
        };
        let week: [Iv; 7] = std::array::from_fn(|i| both(&self.week[i], &o.week[i]));
        if week.iter().all(Vec::is_empty) {
            return WorkCal { week: self.week.clone(), exc: self.exc.clone() };
        }
        let exc = self.exc.keys().chain(o.exc.keys()).map(|&d| (d, both(self.day(d), o.day(d)))).collect();
        WorkCal { week, exc }
    }

    pub fn day(&self, d: NaiveDate) -> &[(i64, i64)] {
        self.exc.get(&d).unwrap_or(&self.week[d.weekday().num_days_from_monday() as usize])
    }

    pub fn is_work_day(&self, d: NaiveDate) -> bool {
        !self.day(d).is_empty()
    }

    /// Moves forward `w` working minutes from `t`. A positive move can end exactly at the end of a shift.
    pub fn add(&self, t: i64, mut w: i64) -> i64 {
        if w <= 0 {
            return if w == 0 { t } else { self.sub(t, -w) };
        }
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        loop {
            for &(s, e) in self.day(d) {
                if e <= m {
                    continue;
                }
                let st = s.max(m);
                if w <= e - st {
                    return at(d) + st + w;
                }
                w -= e - st;
            }
            let Some(x) = next(d) else { return (at(hi()) + DAY).max(t) };
            (d, m) = (x, 0);
        }
    }

    /// Moves back `w` working minutes from `t`.
    pub fn sub(&self, t: i64, mut w: i64) -> i64 {
        if w <= 0 {
            return if w == 0 { t } else { self.add(t, -w) };
        }
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        loop {
            for &(s, e) in self.day(d).iter().rev() {
                if s >= m {
                    continue;
                }
                let en = e.min(m);
                if w <= en - s {
                    return at(d) + en - w;
                }
                w -= en - s;
            }
            let Some(x) = prev(d) else { return at(lo()).min(t) };
            (d, m) = (x, DAY);
        }
    }

    /// Working minutes in [a, b); negative when b is before a. Long spans count whole weeks at
    /// once and then correct for the exception days inside them.
    pub fn between(&self, a: i64, b: i64) -> i64 {
        if b < a {
            return -self.between(b, a);
        }
        let (a, b) = (a.clamp(at(lo()), at(hi()) + DAY), b.clamp(at(lo()), at(hi()) + DAY));
        if b - a > 28 * DAY {
            // [a, w0) and [w1, b) day by day; [w0, w1) as whole weeks starting on a Monday.
            let first = a.div_euclid(DAY) + 1;
            let w0 = (first + (7 - (first + 3).rem_euclid(7)) % 7) * DAY;
            let w1 = w0 + (b - w0).div_euclid(7 * DAY) * 7 * DAY;
            let week: i64 = self.week.iter().map(|iv| iv.iter().map(|(s, e)| e - s).sum::<i64>()).sum();
            let plain = |d: NaiveDate| self.week[d.weekday().num_days_from_monday() as usize].iter().map(|(s, e)| e - s).sum::<i64>();
            let fix: i64 = self.exc.iter().filter(|(d, _)| (w0..w1).contains(&at(**d))).map(|(d, iv)| iv.iter().map(|(s, e)| e - s).sum::<i64>() - plain(*d)).sum();
            return self.between(a, w0) + (w1 - w0) / (7 * DAY) * week + fix + self.between(w1, b);
        }
        let (mut d, last) = (date_of(a), date_of(b));
        let mut n = 0;
        while d <= last {
            let base = at(d);
            for &(s, e) in self.day(d) {
                n += ((base + e).min(b) - (base + s).max(a)).max(0);
            }
            let Some(x) = next(d) else { break };
            d = x;
        }
        n
    }

    /// The next instant at or after `t` that is working time.
    pub fn next_start(&self, t: i64) -> i64 {
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        loop {
            if let Some(&(s, _)) = self.day(d).iter().find(|&&(_, e)| m < e) {
                return at(d) + s.max(m);
            }
            let Some(x) = next(d) else { return (at(hi()) + DAY).max(t) };
            (d, m) = (x, 0);
        }
    }

    /// The last instant at or before `t` that ends working time.
    pub fn prev_end(&self, t: i64) -> i64 {
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        if m == 0 {
            let Some(x) = prev(d) else { return at(lo()).min(t) };
            (d, m) = (x, DAY);
        }
        loop {
            if let Some(&(_, e)) = self.day(d).iter().rev().find(|&&(s, _)| m > s) {
                return at(d) + e.min(m);
            }
            let Some(x) = prev(d) else { return at(lo()).min(t) };
            (d, m) = (x, DAY);
        }
    }

    /// Is `t` inside working time or exactly at the end of a shift?
    pub fn in_or_end(&self, t: i64) -> bool {
        let (d, m) = (date_of(t), t.rem_euclid(DAY));
        self.day(d).iter().any(|&(s, e)| m >= s && m <= e) || (m == 0 && d.pred_opt().is_some_and(|x| self.day(x).last().is_some_and(|&(_, e)| e == DAY)))
    }

    /// End of the working day `d` (or of the last working day before it).
    pub fn end_of(&self, d: NaiveDate) -> i64 {
        self.prev_end(at(d) + DAY)
    }

    /// Working minutes on day `d`.
    pub fn day_minutes(&self, d: NaiveDate) -> i64 {
        self.day(d).iter().map(|(s, e)| e - s).sum()
    }
}
