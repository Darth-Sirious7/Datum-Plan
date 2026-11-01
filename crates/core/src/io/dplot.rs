//! "Open in Datum Plot": a chart's values as a Datum Plot project (.dplot JSON, Plot's format 1).
//! A one-way copy: the plan is never read back, so changes made in Datum Plot stay there.
use crate::cal::{date_of, DAY};
use serde::Deserialize;
use serde_json::{json, Value};

/// One series of a chart: its name, values (gaps as None), colour and whether it is a line.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Series { pub name: String, pub values: Vec<Option<f64>>, pub color: String, pub line: bool }

/// A chart as the window draws it: labels along X, or days (since 1970) for a time axis.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ChartData { pub title: String, pub y: String, pub units: String, pub labels: Vec<String>, pub days: Vec<i64>, pub series: Vec<Series> }

/// The Datum Plot project for a chart of plan `plan` (company and date for its Info sheet).
/// Over time (days given): an XY table, X in days from the first point and each row titled with
/// its date, drawn as lines. Otherwise: a grouped table, one row per label, drawn as bars.
pub fn project(c: &ChartData, plan: &str, company: &str, today: &str) -> Value {
    let rows = c.labels.len().max(c.days.len()).clamp(1, 100_000);
    let xy = c.days.len() == rows;
    let d0 = c.days.first().copied().unwrap_or(0);
    let first = date_of(d0 * DAY);
    let x: Vec<Option<f64>> = if xy { c.days.iter().map(|d| Some((d - d0) as f64)).collect() } else { vec![None; rows] };
    let fit = |v: &[Option<f64>]| (0..rows).map(|k| v.get(k).copied().flatten().filter(|x| x.is_finite())).collect::<Vec<_>>();
    let series: Vec<&Series> = c.series.iter().take(1000).collect();
    let cols: Vec<Vec<Option<f64>>> = std::iter::once(x).chain(series.iter().map(|s| fit(&s.values))).collect();
    let titles: Vec<String> = (0..rows).map(|k| if xy { date_of(c.days[k] * DAY).format("%Y-%m-%d").to_string() } else { c.labels.get(k).cloned().unwrap_or_default() }).collect();
    let sets: Vec<Value> = series.iter().map(|s| json!({ "title": s.name, "entry": "rep", "reps": 1, "role": "", "units": "", "comment": "", "formula": "" })).collect();
    let styles: Vec<Value> = series.iter().map(|s| json!({ "show": true, "color": s.color, "symbol": if xy { "none" } else { "" }, "size": 4.0, "filled": true, "line": xy || s.line, "width": 1.5, "right": false })).collect();
    let name = format!("{plan} - {}", c.title);
    let note = format!("Copied from the Datum Plan plan \"{plan}\" on {today}. It is a copy: changes made here do not change the plan.");
    json!({
        "format": 1, "name": name, "nextId": 3, "analyses": [], "layouts": [], "ui": {},
        "info": { "title": c.title, "experiment": format!("Datum Plan: {plan}"), "researcher": "", "organisation": company, "date": today, "notes": note },
        "tables": [{
            "id": 1, "name": c.title, "kind": if xy { "xy" } else { "grouped" },
            "xTitle": if xy { format!("Days from {}", first.format("%d %b %Y")) } else { String::new() }, "xUnits": "",
            "yTitle": c.y, "yUnits": c.units, "rows": rows, "rowTitles": titles, "sets": sets, "cols": cols, "excluded": [], "style": {}, "notes": note,
        }],
        "graphs": [{ "id": 2, "table": 1, "name": c.title, "settings": { "title": c.title, "showTitle": true, "line": xy, "column": "bars", "points": "mean", "error": "none", "width": 160.0, "height": 100.0, "sets": styles } }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_time_chart_becomes_an_xy_table_and_a_bar_chart_a_grouped_one() {
        let s = |n: &str, v: &[Option<f64>]| Series { name: n.into(), values: v.to_vec(), color: "#3B82F6".into(), line: true };
        let c = ChartData { title: "S-curve".into(), y: "Cumulative progress".into(), units: "%".into(), days: vec![20_738, 20_745, 20_752], series: vec![s("Planned", &[Some(0.0), Some(40.0), Some(100.0)]), s("Actual", &[Some(0.0), Some(35.0), None])], ..Default::default() };
        let p = project(&c, "Fit-out", "Acme", "2026-11-01");
        let t = &p["tables"][0];
        assert_eq!((t["kind"].as_str(), t["rows"].as_u64()), (Some("xy"), Some(3)));
        assert_eq!(t["cols"][0], json!([0.0, 7.0, 14.0]));
        assert_eq!(t["cols"][2][2], Value::Null);
        assert_eq!(t["rowTitles"][0], "2026-10-12");
        assert_eq!(t["xTitle"], "Days from 12 Oct 2026");
        let b = ChartData { title: "By phase".into(), labels: vec!["Design".into(), "Build".into()], series: vec![s("Actual", &[Some(100.0), Some(40.0), Some(9.0)])], ..Default::default() };
        let q = project(&b, "Fit-out", "", "2026-11-01");
        assert_eq!(q["tables"][0]["kind"], "grouped");
        assert_eq!(q["tables"][0]["cols"][1], json!([100.0, 40.0]), "values beyond the labels are left out");
        assert_eq!(q["tables"][0]["rowTitles"], json!(["Design", "Build"]));
    }
}
