//! Sheets: the numbers a planner shows a client, checked against what a spreadsheet gives.
use crate::{doc::{Doc, Op}, fields::Calc, *};
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
/// Three tasks of 10, 5 and 0 days in a row from Monday 2 Nov 2026, baselined; Design half done at
/// the status date (Friday 6 Nov).
fn small_plan() -> Doc {
    let mut p = Project::new("Small", d(2026, 11, 2));
    for (n, days) in [("Design", 10), ("Build", 5), ("Handover", 0)] { let t = p.new_task(n, 0, days * 480); p.tasks.push(t); }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for k in 1..3 { p.tasks[k].preds = crate::parse::preds(&ids[k - 1].to_string(), &ids, 480).unwrap(); }
    let mut doc = Doc::new(p);
    let uids = doc.p.tasks.iter().map(|t| t.uid).collect();
    doc.apply(Op::Baseline { uids, clear: false, n: 0 }, "%d/%m/%Y").unwrap();
    doc.p.tasks[0].pct = 50;
    doc.p.status = Some(d(2026, 11, 6));
    doc
}
fn add(doc: &mut Doc, name: &str, cells: &[(&str, &str)]) {
    let s = SheetData { name: name.into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() };
    doc.apply(Op::Sheet { k: None, sheet: Some(s) }, "%d/%m/%Y").unwrap();
}
fn set_fmt(doc: &mut Doc, k: usize, a1: &str, f: &str) {
    doc.apply(Op::SheetCells { k, cells: vec![(a1.into(), String::new())], fmt: Some(f.into()) }, "%d/%m/%Y").unwrap();
}
/// Sheet `k` as shown, in a plan whose dates are written `fmt`: address → (text, number).
fn shown_in(doc: &Doc, k: usize, fmt: &str) -> HashMap<String, (String, Option<f64>)> {
    let c = Calc::new(&doc.p, fmt, d(2026, 11, 2));
    crate::sheet::sheet_view(&c, k).unwrap().cells.into_iter().map(|x| (x.0, (x.2, x.4))).collect()
}
/// One sheet of `cells` on the small plan, as shown: address → text.
fn sheet(fmt: &str, cells: &[(&str, &str)]) -> HashMap<String, String> {
    let mut doc = small_plan();
    add(&mut doc, "S", cells);
    shown_in(&doc, 0, fmt).into_iter().map(|(a, (t, _))| (a, t)).collect()
}
fn num(fmt: &str, cells: &[(&str, &str)], at: &str) -> Option<f64> {
    let mut doc = small_plan();
    add(&mut doc, "S", cells);
    shown_in(&doc, 0, fmt).get(at).and_then(|x| x.1)
}
/// Formulas in column B over the data in A1:C5, each checked against its expected text.
fn check(fmt: &str, data: &[(&str, &str)], cases: &[(&str, &str)]) {
    let cells: Vec<(String, String)> = data.iter().map(|(a, b)| (a.to_string(), b.to_string())).chain(cases.iter().enumerate().map(|(i, (f, _))| (format!("E{}", i + 1), f.to_string()))).collect();
    let v = sheet(fmt, &cells.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect::<Vec<_>>());
    let bad: Vec<String> = cases.iter().enumerate().filter_map(|(i, (f, want))| {
        let got = v.get(&format!("E{}", i + 1)).cloned().unwrap_or_default();
        (got != *want).then(|| format!("{f}: got {got:?}, a spreadsheet gives {want:?}"))
    }).collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// --- everyday calculations ------------------------------------------------------------------------

#[test]
fn everyday_functions_over_blanks_text_and_errors() {
    // A: 10, blank, text, 20, an error; B: statuses; C: amounts.
    let data = [("A1", "10"), ("A3", "x"), ("A4", "20"), ("A5", "=1/0"), ("B1", "Done"), ("B2", "Open"), ("B3", "Done"), ("B4", "Open"), ("C1", "5"), ("C2", "7"), ("C3", "9"), ("C4", "11")];
    check("%d/%m/%Y", &data, &[
        ("=SUM(A1:A4)", "30"), ("=AVERAGE(A1:A4)", "15"), ("=COUNT(A1:A4)", "2"), ("=COUNTA(A1:A4)", "3"), ("=COUNTBLANK(A1:A4)", "1"),
        ("=SUM(A1:A5)", "#DIV/0!"), ("=A1+A3", "#VALUE!"), ("=A2", "0"), ("=A2+1", "1"), ("=IF(A2=\"\",\"blank\",\"no\")", "blank"), ("=IF(A2=0,\"zero\",\"no\")", "zero"),
        ("=COUNTIF(B1:B4,\"Done\")", "2"), ("=COUNTIF(C1:C4,\">=7\")", "3"), ("=SUMIF(B1:B4,\"Done\",C1:C4)", "14"), ("=SUMIF(C1:C4,\">6\")", "27"),
        ("=SUMIFS(C1:C4,B1:B4,\"Open\",C1:C4,\">8\")", "11"), ("=COUNTIFS(B1:B4,\"Done\",C1:C4,\">6\")", "1"), ("=AVERAGEIF(B1:B4,\"Done\",C1:C4)", "7"),
        ("=IF(A1>5,\"big\",\"small\")", "big"), ("=IFERROR(A5,0)", "0"), ("=IFNA(VLOOKUP(\"Zzz\",B1:C4,2,FALSE),\"-\")", "-"),
        ("=ROUND(2.5,0)", "3"), ("=ROUND(-2.5,0)", "-3"), ("=ROUND(1.005,2)", "1.01"), ("=ROUNDUP(2.1,0)", "3"), ("=ROUNDDOWN(-2.9,0)", "-2"), ("=MROUND(7,5)", "5"),
        ("=INT(-2.5)", "-3"), ("=MOD(-3,2)", "1"), ("=TRUNC(-2.7)", "-2"), ("=-2^2", "4"), ("=0.1+0.2", "0.3"), ("=10/3", "3.333333333"), ("=C1/C4", "0.454545455"),
        ("=VLOOKUP(\"Open\",B1:C4,2,FALSE)", "7"), ("=VLOOKUP(\"Zzz\",B1:C4,2,FALSE)", "#N/A"), ("=XLOOKUP(\"Open\",B1:B4,C1:C4)", "7"),
        ("=XLOOKUP(\"Zzz\",B1:B4,C1:C4,\"none\")", "none"), ("=INDEX(C1:C4,MATCH(\"Open\",B1:B4,0))", "7"), ("=MATCH(8,C1:C4,1)", "2"),
        ("=SUMPRODUCT(C1:C4,C1:C4)", "276"), ("=MEDIAN(C1:C4)", "8"), ("=STDEV(C1:C4)", "2.581988897"), ("=\"5\"+1", "6"), ("=\"a\"=\"A\"", "TRUE"),
        ("=TEXT(1234.5,\"#,##0.00\")", "1,234.50"), ("=TEXT(0.256,\"0.0%\")", "25.6%"), ("=TEXTJOIN(\",\",TRUE,B1:B4)", "Done,Open,Done,Open"), ("=C2/C4", "0.636363636"),
        ("=C1/SUM(C1:C4)", "0.15625"), ("=ROUND(C1/SUM(C1:C4)*100,1)", "15.6"),
    ]);
}

#[test]
fn date_functions_give_dates() {
    check("%d/%m/%Y", &[], &[
        ("=DATE(2026,3,5)", "05/03/2026"), ("=TEXT(DATE(2026,3,5),\"dd/mm/yyyy\")", "05/03/2026"), ("=TEXT(DATE(2026,3,5),\"mmm yyyy\")", "Mar 2026"),
        ("=NETWORKDAYS(DATE(2026,11,2),DATE(2026,11,13))", "10"), ("=NETWORKDAYS.INTL(DATE(2026,11,2),DATE(2026,11,13),7)", "9"),
        ("=WORKDAY(DATE(2026,11,6),1)", "09/11/2026"), ("=WORKDAY(DATE(2026,11,6),-1)", "05/11/2026"), ("=EDATE(DATE(2026,1,31),1)", "28/02/2026"),
        ("=EOMONTH(DATE(2026,2,10),0)", "28/02/2026"), ("=DATEDIF(DATE(2026,1,1),DATE(2026,3,1),\"d\")", "59"), ("=DATEDIF(DATE(2026,1,15),DATE(2026,3,1),\"m\")", "1"),
        ("=DAYS(DATE(2026,12,25),DATE(2026,11,4))", "51"), ("=WEEKDAY(DATE(2026,11,4))", "4"), ("=WEEKNUM(DATE(2026,11,4))", "45"), ("=YEARFRAC(DATE(2026,1,1),DATE(2026,7,1))", "0.5"),
        ("=DATEVALUE(\"2026-03-05\")", "05/03/2026"), ("=DATE(2026,3,5)-DATE(2026,3,1)", "4"), ("=DATE(2026,3,5)+10", "15/03/2026"),
        ("=TIME(12,30,0)", "12:30"),
    ]);
}

// --- typed values ---------------------------------------------------------------------------------

#[test]
fn typed_numbers_percentages_and_negatives() {
    let typed = [("25%", 0.25, "25%"), ("12.50%", 0.125, "12.50%"), ("1,234", 1234.0, "1,234"), ("-1,234.5", -1234.5, "-1,234.50"), ("-5", -5.0, "-5"), ("(5)", -5.0, "-5"), ("$1,234.50", 1234.5, "$1,234.50"), (" 7 ", 7.0, "7")];
    let cells: Vec<(String, String)> = typed.iter().enumerate().flat_map(|(i, (t, _, _))| [(format!("A{}", i + 1), t.to_string()), (format!("B{}", i + 1), format!("=A{}*2", i + 1))]).collect();
    let mut doc = small_plan();
    add(&mut doc, "S", &cells.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect::<Vec<_>>());
    let v = shown_in(&doc, 0, "%d/%m/%Y");
    for (i, (t, n, text)) in typed.iter().enumerate() {
        assert_eq!(v[&format!("A{}", i + 1)], (text.to_string(), Some(*n)), "{t} typed");
        assert_eq!(v[&format!("B{}", i + 1)].1, Some(n * 2.0), "{t} doubled");
    }
}

#[test]
fn amounts_typed_with_a_currency_symbol_are_numbers() {
    // A planner in a euro or pound plan types the amount as a client reads it.
    let v = sheet("%d/%m/%Y", &[("A1", "€1,234.50"), ("A2", "£9"), ("A3", "-€12"), ("A4", "1.234,5 €"), ("A5", "12 €"), ("B1", "=SUM(A1:A3)"), ("B2", "=A5*2")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str(), v["A3"].as_str(), v["A5"].as_str()), ("€1,234.50", "£9", "-€12", "12 €"));
    assert_eq!((v["B1"].as_str(), v["B2"].as_str()), ("1231.5", "24"), "amounts with a symbol add up like plain numbers");
    assert_eq!(v["A4"], "1.234,5 €", "a form the plan does not write stays text");
}

#[test]
fn two_digit_years_read_as_a_spreadsheet_reads_them() {
    // 00–29 are 2000–2029, 30–99 are 1930–1999; a year is never 0026.
    let v = sheet("%d/%m/%Y", &[("A1", "1/2/26"), ("A2", "1/2/45"), ("A3", "=A1+1")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str(), v["A3"].as_str()), ("01/02/2026", "01/02/1945", "02/02/2026"));
    assert_eq!(num("%m/%d/%Y", &[("A1", "1/2/26")], "A1"), Some(46024.0), "2 January 2026 in a month-first plan");
}

#[test]
fn slash_dates_follow_the_plans_day_order() {
    // The default plan format (24-Oct-26) writes the day first, so 05/01/2026 is 5 January.
    let v = sheet("%d-%b-%y", &[("A1", "13/01/2026"), ("A2", "05/01/2026"), ("A3", "=A1-A2"), ("A4", "5.1.2026")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str(), v["A3"].as_str(), v["A4"].as_str()), ("13-Jan-26", "05-Jan-26", "8", "05-Jan-26"));
    let y = chrono::Local::now().year();
    assert_eq!(sheet("%d-%b-%y", &[("A1", "3/4")])["A1"], format!("03-Apr-{:02}", y % 100), "day and month only: this year");
    // A month-first plan reads them month first; 13/01/2026 is then no date at all.
    let v = sheet("%m/%d/%Y", &[("A1", "13/01/2026"), ("A2", "05/01/2026")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str()), ("13/01/2026", "05/01/2026"));
    assert_eq!(num("%m/%d/%Y", &[("A1", "05/01/2026")], "A1"), Some(46143.0), "1 May");
    assert_eq!(num("%m/%d/%Y", &[("A1", "13/01/2026")], "A1"), None);
    assert_eq!(num("%Y-%m-%d", &[("A1", "2026-03-05")], "A1"), Some(46086.0));
    assert_eq!(num("%Y-%m-%d", &[("A1", "31/01/2026")], "A1"), Some(46053.0), "a year-first plan reads slashes day first, as its fill handle does");
    // Version numbers and decimals stay what they are.
    let v = sheet("%d/%m/%Y", &[("A1", "1.2.3"), ("A2", "1.5"), ("A3", "=A2*2")]);
    assert_eq!((v["A1"].as_str(), v["A3"].as_str()), ("1.2.3", "3"));
}

#[test]
fn dates_written_in_criteria_follow_the_plans_day_order() {
    // In a day-first plan "05/01/2026" is 5 January in a criterion too, as in a cell.
    let data = [("A1", "05/01/2026"), ("A2", "13/01/2026"), ("C1", "10"), ("C2", "20")];
    check("%d/%m/%Y", &data, &[
        ("=COUNTIF(A1:A2,\"05/01/2026\")", "1"), ("=COUNTIF(A1:A2,\">\"&\"10/01/2026\")", "1"), ("=COUNTIF(A1:A2,\">=13/01/2026\")", "1"), ("=SUMIFS(C1:C2,A1:A2,\"<10/01/2026\")", "10"),
        ("=DATEVALUE(\"13/01/2026\")", "13/01/2026"), ("=\"13/01/2026\"+1", "46036"), ("=\"Due \"&\"13/01/2026\"", "Due 13/01/2026"), ("=COUNTIF(A1:A2,\">\"&DATE(2026,1,10))", "1"),
    ]);
    check("%m/%d/%Y", &[("A1", "05/01/2026")], &[("=COUNTIF(A1,\"05/01/2026\")", "1"), ("=DATEVALUE(\"05/01/2026\")", "05/01/2026")]);
}

#[test]
fn a_date_minus_a_date_is_days_not_a_date() {
    // Finish − Start on typed dates: the number of days, not "07/01/1900".
    let v = sheet("%d/%m/%Y", &[("A1", "05/01/2026"), ("A2", "12/01/2026"), ("A3", "=A2-A1"), ("A4", "=A2+7"), ("A5", "=MAX(A1:A2)"), ("A6", "=A2-A1+1"), ("A7", "=A4-7"), ("A8", "=A3*2")]);
    assert_eq!((v["A3"].as_str(), v["A4"].as_str(), v["A5"].as_str(), v["A6"].as_str(), v["A7"].as_str(), v["A8"].as_str()), ("7", "19/01/2026", "12/01/2026", "8", "12/01/2026", "14"));
}

// --- the plan's own figures -----------------------------------------------------------------------

#[test]
fn plan_dates_are_the_days_the_task_sheet_shows() {
    // Design 2–13 Nov, Build 16–20 Nov, Handover 20 Nov.
    let v = sheet("%d/%m/%Y", &[
        ("A1", "=COUNTIFS(Tasks[Finish],\"<=\"&DATE(2026,11,13))"), ("A2", "=ProjectFinish=MAX(Tasks[Finish])"), ("A3", "=XLOOKUP(ProjectFinish,Tasks[Finish],Tasks[Name])"),
        ("A4", "=INDEX(Tasks[Finish],1)-INDEX(Tasks[Start],1)+1"), ("A5", "=ProjectFinish-ProjectStart"), ("A6", "=COUNTIF(Tasks[Start],DATE(2026,11,16))"),
        ("A7", "=ProjectStart"), ("A8", "=SUMPRODUCT((Tasks[Finish]>=DATE(2026,11,20))*1)"), ("A9", "=BaselineFinish=ProjectFinish"),
    ]);
    assert_eq!(v["A1"], "1", "tasks finishing by 13 Nov");
    assert_eq!((v["A2"].as_str(), v["A3"].as_str()), ("TRUE", "Build"), "the project finish is a task's finish");
    assert_eq!((v["A4"].as_str(), v["A5"].as_str(), v["A6"].as_str()), ("12", "18", "1"), "calendar days");
    assert_eq!((v["A7"].as_str(), v["A8"].as_str(), v["A9"].as_str()), ("02/11/2026", "2", "TRUE"));
}

#[test]
fn plan_names_on_a_small_plan() {
    let v = sheet("%d/%m/%Y", &[
        ("A1", "=ProjectStart"), ("A2", "=ProjectFinish"), ("A3", "=StatusDate"), ("A4", "=BaselineFinish"), ("A5", "=FinishVariance"), ("A6", "=PercentComplete"),
        ("A7", "=PlannedPercent"), ("A8", "=TaskCount"), ("A9", "=TasksDone"), ("A10", "=MilestoneCount"), ("A11", "=CriticalCount"), ("A12", "=SUM(Tasks[Duration])"),
        ("A13", "=XLOOKUP(\"Design\",Tasks[Name],Tasks[% Complete])"), ("A14", "=ProjectName"), ("A15", "=ROWS(Tasks)"), ("A16", "=PercentComplete/100"),
        ("A17", "=XLOOKUP(\"Design\",Tasks[Name],Tasks[Remaining Duration])"), ("A18", "=IF(ProjectFinish>BaselineFinish,\"Late\",\"On time\")"),
    ]);
    let want = [("A1", "02/11/2026"), ("A2", "20/11/2026"), ("A3", "06/11/2026"), ("A4", "20/11/2026"), ("A5", "0"), ("A6", "33%"), ("A7", "33%"), ("A8", "3"), ("A9", "0"), ("A10", "1"),
        ("A11", "3"), ("A12", "15"), ("A13", "50%"), ("A14", "Small"), ("A15", "3"), ("A16", "0.333333333"), ("A17", "5"), ("A18", "On time")];
    let bad: Vec<_> = want.iter().filter(|(a, w)| v[*a] != *w).map(|(a, w)| format!("{a}: {:?} ≠ {w:?}", v[*a])).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn earned_value_names_agree_with_the_task_columns() {
    let mut p = small_plan().p;
    p.tasks[0].fixed_cost = 1000.0;
    p.tasks[1].fixed_cost = 500.0;
    let mut doc = Doc::new(p);
    let uids = doc.p.tasks.iter().map(|t| t.uid).collect();
    doc.apply(Op::Baseline { uids, clear: false, n: 0 }, "%d/%m/%Y").unwrap();
    add(&mut doc, "EV", &[("A1", "=TotalCost"), ("A2", "=PlannedValue"), ("A3", "=EarnedValue"), ("A4", "=SPI"), ("A5", "=CPI"), ("A6", "=SUM(Tasks[Earned Value])=EarnedValue"),
        ("A7", "=SUM(Tasks[Planned Value])=PlannedValue"), ("A8", "=ROUND(EarnedValue/PlannedValue,2)=ROUND(SPI,2)"), ("A9", "=SUM(Tasks[Cost])")]);
    let v = shown_in(&doc, 0, "%d/%m/%Y");
    let t = |a: &str| v[a].0.clone();
    assert_eq!((t("A1"), t("A2"), t("A3"), t("A4"), t("A9")), ("1500".into(), "500".into(), "500".into(), "1".into(), "1500".into()), "half of Design (1,000) planned and earned by 6 Nov");
    assert_eq!((t("A6"), t("A7"), t("A8")), ("TRUE".into(), "TRUE".into(), "TRUE".into()));
}

#[test]
fn date_arithmetic_on_plan_dates_shows_a_date() {
    let v = sheet("%d/%m/%Y", &[("A1", "=ProjectFinish+7"), ("A2", "=Plan!H2+1"), ("A3", "=XLOOKUP(\"Design\",Tasks[Name],Tasks[Finish])-XLOOKUP(\"Design\",Tasks[Name],Tasks[Start])"),
        ("A4", "=INDEX(Tasks[Duration],MATCH(MAX(Tasks[Finish]),Tasks[Finish],0))"), ("A5", "=(ProjectFinish)-14"), ("A6", "=StatusDate-ProjectStart"), ("A7", "=IF(TRUE,ProjectFinish+1,0)"), ("A8", "=MAX(Tasks[Finish])+1")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str(), v["A5"].as_str(), v["A7"].as_str(), v["A8"].as_str()), ("27/11/2026", "14/11/2026", "06/11/2026", "21/11/2026", "21/11/2026"));
    assert_eq!((v["A3"].as_str(), v["A4"].as_str(), v["A6"].as_str()), ("11", "5", "4"), "days and durations are numbers");
}

#[test]
fn a_rounded_plan_percentage_keeps_its_decimals() {
    let v = sheet("%d/%m/%Y", &[("A1", "=ROUND(PercentComplete,1)"), ("A2", "=ROUND(PercentComplete,0)"), ("A3", "=PercentComplete"), ("A4", "=ROUND(PlannedPercent,2)")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str(), v["A3"].as_str(), v["A4"].as_str()), ("33.3%", "33%", "33%", "33.33%"));
}

#[test]
fn a_blank_plan_date_shows_blank() {
    // Design has no actual start: a lookup gives 0, which is no date to show a client.
    let v = sheet("%d/%m/%Y", &[("A1", "=XLOOKUP(\"Design\",Tasks[Name],Tasks[Actual Start])"), ("A2", "=XLOOKUP(\"Design\",Tasks[Name],Tasks[Actual Finish])")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str()), ("", ""));
}

#[test]
fn spilled_plan_dates_show_as_dates() {
    let v = sheet("%d/%m/%Y", &[("A1", "=SORT(Tasks[Finish])"), ("B1", "=FILTER(Tasks[Name],Tasks[Duration]>0)"), ("C1", "=Tasks[% Complete]")]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str(), v["A3"].as_str()), ("13/11/2026", "20/11/2026", "20/11/2026"), "every spilled date, not only the first");
    assert_eq!((v["B2"].as_str(), v["C1"].as_str(), v["C2"].as_str()), ("Build", "50%", "0%"));
    // A dashboard table of dates.
    let doc = small_plan();
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    let r = crate::sheet::eval(&c, &[("=FILTER(Tasks[Finish],Tasks[Duration]>0)".into(), String::new())]);
    assert_eq!(r[0].iter().map(|row| row[0].0.as_str()).collect::<Vec<_>>(), ["13/11/2026", "20/11/2026"]);
}

#[test]
fn dates_read_from_another_tab_stay_dates() {
    let mut doc = small_plan();
    add(&mut doc, "Dates", &[("A1", "05/01/2026"), ("A2", "=DATE(2026,2,1)"), ("A3", "=A2")]);
    add(&mut doc, "Use", &[("A1", "=Dates!A1+7"), ("A2", "=Dates!A3+1"), ("A3", "=Dates!A2-Dates!A1"), ("A4", "=MAX(Dates!A1:A3)")]);
    let v = shown_in(&doc, 1, "%d/%m/%Y");
    let t = |a: &str| v[a].0.clone();
    assert_eq!((t("A1"), t("A2"), t("A3"), t("A4")), ("12/01/2026".into(), "02/02/2026".into(), "27".into(), "01/02/2026".into()));
}

#[test]
fn formula_columns_count_whole_days() {
    let mut doc = small_plan();
    doc.apply(Op::FieldDef { def: CustomDef { key: "number1".into(), alias: "Calendar days".into(), formula: "=[Finish]-[Start]+1".into(), ..Default::default() } }, "%d/%m/%Y").unwrap();
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    assert_eq!(c.val(0, "c:number1"), crate::formula::F::Num(12.0), "Design runs 2–13 Nov: 12 calendar days");
}

// --- references -----------------------------------------------------------------------------------

#[test]
fn fill_and_paste_move_relative_references_only() {
    use crate::sheet::shift;
    for (f, dr, dc, want) in [("=A1+$B$1+B$2+$C3", 1, 1, "=B2+$B$1+C$2+$C4"), ("=SUM(A1:A3)", 2, 0, "=SUM(A3:A5)"), ("=A1", -1, 0, "=#REF!"), ("=Data!A1", 1, 0, "=Data!A2"),
        ("=SUM(A:A)", 1, 1, "=SUM(B:B)"), ("=Tasks[Cost]", 3, 3, "=Tasks[Cost]"), ("=A1048576", 1, 0, "=#REF!"), ("=\"A1\"&A1", 1, 0, "=\"A1\"&A2"), ("='My Data'!B2*2", 1, 1, "='My Data'!C3*2"), ("12", 5, 5, "12")] {
        assert_eq!(shift(f, dr, dc), want, "{f} moved {dr},{dc}");
    }
}

#[test]
fn renamed_and_deleted_sheets_follow_in_formulas() {
    use crate::sheet::rename;
    let order: Vec<String> = ["Plan", "Resources", "Data", "Data2"].iter().map(|x| x.to_string()).collect();
    for (f, new, want) in [("=Data!A1", "My Data", "='My Data'!A1"), ("=Data2!A1+Data!A1", "D", "=Data2!A1+D!A1"), ("=data!A1", "Données é", "='Données é'!A1"), ("=Data!A1", "Q1'2026", "='Q1''2026'!A1"),
        ("=Data!A1", "2026", "='2026'!A1"), ("=Data!A1", "A1", "='A1'!A1"), ("=\"Data!A1\"", "X", "=\"Data!A1\""), ("=Data!A1", "", "=#REF!"), ("=SUM(Data:Data2!A1)", "", "=SUM(Data2!A1)")] {
        assert_eq!(rename(f, "Data", new, &order), want, "{f} after Data → {new:?}");
    }
}

#[test]
fn inserted_and_deleted_rows_move_references() {
    use crate::sheet::adjust;
    use gridcraft_formula::adjust::{Axis::*, Edit::*};
    for (f, host, e, want) in [("=SUM(A1:A5)", "S", Insert { axis: Rows, at: 2, count: 2 }, "=SUM(A1:A7)"), ("=SUM(A1:A5)", "S", Delete { axis: Rows, at: 1, count: 2 }, "=SUM(A1:A3)"),
        ("=SUM(A2:A3)", "S", Delete { axis: Rows, at: 1, count: 2 }, "=SUM(#REF!)"), ("=$A$5", "S", Insert { axis: Rows, at: 0, count: 1 }, "=$A$6"), ("=Other!A5", "S", Insert { axis: Rows, at: 0, count: 1 }, "=Other!A5"),
        ("=A5", "Other", Insert { axis: Rows, at: 0, count: 1 }, "=A5"), ("=s!A5", "Other", Insert { axis: Rows, at: 0, count: 1 }, "=s!A6"), ("=SUM(B:B)", "S", Delete { axis: Cols, at: 1, count: 1 }, "=SUM(#REF!)"),
        ("=SUM(A1:A5)", "S", Insert { axis: Rows, at: 5, count: 1 }, "=SUM(A1:A5)"), ("=ProjectFinish+A1", "S", Insert { axis: Rows, at: 0, count: 1 }, "=ProjectFinish+A2")] {
        assert_eq!(adjust(f, host, "S", &e), want, "{f} on {host} after {e:?}");
    }
}

#[test]
fn cross_sheet_unicode_names_rename_insert_delete() {
    let mut doc = small_plan();
    add(&mut doc, "Données é ✓", &[("A1", "5"), ("A2", "=A1*2")]);
    add(&mut doc, "Use", &[("E1", "='Données é ✓'!A2+1"), ("F1", "=SUM('Données é ✓'!A1:A2)"), ("G1", "=Empty!A1"), ("H1", "=SUM(Empty!A1:B9)"), ("I1", "=Nosuch!A1"), ("J1", "=COUNTA(Empty!A:A)")]);
    add(&mut doc, "Empty", &[]);
    let v = shown_in(&doc, 1, "%d/%m/%Y");
    let t = |a: &str| v[a].0.clone();
    assert_eq!((t("E1"), t("F1"), t("G1"), t("H1"), t("I1"), t("J1")), ("11".into(), "15".into(), "0".into(), "0".into(), "#REF!".into(), "0".into()));
    assert!(shown_in(&doc, 2, "%d/%m/%Y").is_empty(), "an empty sheet shows nothing");
    let mut s = doc.p.sheets[0].clone();
    s.name = "Q1 data".into();
    doc.apply(Op::Sheet { k: Some(0), sheet: Some(s) }, "%d/%m/%Y").unwrap();
    doc.apply(Op::SheetInsert { k: 0, cols: false, at: 0, count: 1 }, "%d/%m/%Y").unwrap();
    assert_eq!(doc.p.sheets[1].cells["F1"], "=SUM('Q1 data'!A2:A3)");
    assert_eq!(shown_in(&doc, 1, "%d/%m/%Y")["E1"].0, "11", "the value follows the rename and the inserted row");
    doc.apply(Op::Sheet { k: Some(0), sheet: None }, "%d/%m/%Y").unwrap();
    assert_eq!((doc.p.sheets[0].cells["E1"].as_str(), doc.p.sheets[0].cells["F1"].as_str()), ("=#REF!+1", "=SUM(#REF!)"));
}

// --- edge cases -----------------------------------------------------------------------------------

#[test]
fn circular_references_stop_with_an_error() {
    let v = sheet("%d/%m/%Y", &[("A1", "=B1+1"), ("B1", "=A1+1"), ("C1", "=SUM(C:C)"), ("D1", "=D1"), ("E1", "=5")]);
    assert_eq!((v["A1"].as_str(), v["B1"].as_str(), v["C1"].as_str(), v["D1"].as_str(), v["E1"].as_str()), ("#CIRC!", "#CIRC!", "#CIRC!", "#CIRC!", "5"));
}

#[test]
fn long_formulas_work() {
    let v = sheet("%d/%m/%Y", &[("A1", &format!("={}", vec!["1"; 3000].join("+"))), ("A2", &format!("={}1{}", "(".repeat(60), ")".repeat(60))), ("A3", &format!("={}1{}", "SUM(".repeat(64), ")".repeat(64)))]);
    assert_eq!((v["A1"].as_str(), v["A2"].as_str(), v["A3"].as_str()), ("3000", "1", "1"));
}

#[test]
fn formulas_nested_past_the_spreadsheet_limit_are_refused_not_a_crash() {
    // A spreadsheet allows 64 levels; deeper ones (typed, pasted or in a workbook) once overflowed the stack.
    let deep = format!("={}1{}", "SUM(".repeat(300), ")".repeat(300));
    let v = sheet("%d/%m/%Y", &[("A1", &deep), ("A2", "=1+1")]);
    assert!(v["A1"].starts_with('#'), "{:?}", v["A1"]);
    assert_eq!(v["A2"], "2");
    assert_eq!(crate::sheet::shift(&deep, 1, 1), deep);
    assert_eq!(crate::sheet::rename(&deep, "A", "B", &[]), deep);
    assert!(crate::sheet::check(&deep, |_| true).is_err());
    let mut doc = small_plan();
    add(&mut doc, "Deep", &[("A1", &deep)]);
    doc.apply(Op::SheetInsert { k: 0, cols: false, at: 0, count: 1 }, "%d/%m/%Y").unwrap();
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    assert!(crate::sheet::eval(&c, &[(deep.clone(), String::new())])[0][0][0].0.starts_with('#'));
}

#[test]
fn long_operator_chains_are_refused_not_a_crash() {
    let chains = [vec!["1"; 4000].join("+"), vec!["2"; 4000].join("^"), vec!["\"a\""; 2000].join("&"), vec!["A1"; 2700].join(":"), format!("SUM({})", vec!["A1"; 2700].join(",")), vec!["1"; 4000].join("=")];
    for f in [format!("={}1", "-".repeat(4000)), format!("=1{}", "%".repeat(4000)), format!("={}", vec!["(1"; 2000].join("+") + &")".repeat(2000)), format!("={}1", "+-".repeat(2000))].into_iter().chain(chains.iter().map(|c| format!("={c}"))) {
        let v = sheet("%d/%m/%Y", &[("A1", &f), ("A2", "=1")]);
        assert_eq!(v["A2"], "1");
        let _ = crate::sheet::shift(&f, 1, 1);
    }
}

#[test]
fn a_big_sheet_works_out_its_formats_quickly() {
    // Most of the time goes to the engine's own running MAX; the formats add little.
    let cells: Vec<(String, String)> = (1..=1500).flat_map(|i| [(format!("A{i}"), format!("{i}")), (format!("B{i}"), format!("=MAX(A$1:A{i})")), (format!("C{i}"), format!("=B{i}+MIN(A:A)"))]).collect();
    let t = std::time::Instant::now();
    let v = sheet("%d/%m/%Y", &cells.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect::<Vec<_>>());
    println!("4500 cells in {:?}", t.elapsed());

    assert_eq!((v["B1500"].as_str(), v["C1500"].as_str()), ("1500", "1501"));
}

// --- number formats -------------------------------------------------------------------------------

#[test]
fn number_formats_show_as_a_spreadsheet_shows_them() {
    let fmts = [("1234.5", "#,##0", "1,235"), ("1234.5", "#,##0.00", "1,234.50"), ("-1234.5", "#,##0.00", "-1,234.50"), ("0.256", "0%", "26%"), ("0.256", "0.0%", "25.6%"),
        ("25.6", "0\"%\"", "26%"), ("1234.5", "money", "$1,234.50"), ("-1234.5", "money", "-$1,234.50"), ("46086", "date", "05/03/2026"), ("46086", "d mmmm yyyy", "5 March 2026"),
        ("46086", "mmm-yy", "Mar-26"), ("-5", "0.00;[Red](0.00)", "(5.00)"), ("1234567", "#,##0,\"K\"", "1,235K"), ("0.5", "h:mm", "12:00"), ("1234.5", "0.00E+00", "1.23E+03"),
        ("12", "00000", "00012"), ("001", "@", "001"), ("0", "#,##0;-#,##0;\"-\"", "-"), ("0.25", "pct", "25.0%"), ("=1/0", "#,##0", "#DIV/0!"), ("2.5", "0", "3"), ("-0.4", "0", "0"),
        ("1234.5", "#,##0.00 [$€-407]", "1,234.50 €"), ("abc", "0.00", "abc")];
    let mut doc = small_plan();
    let cells: Vec<(String, String)> = fmts.iter().enumerate().map(|(i, (v, _, _))| (format!("A{}", i + 1), v.to_string())).collect();
    add(&mut doc, "F", &cells.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect::<Vec<_>>());
    for (i, (_, f, _)) in fmts.iter().enumerate() { set_fmt(&mut doc, 0, &format!("A{}", i + 1), f); }
    let v = shown_in(&doc, 0, "%d/%m/%Y");
    let bad: Vec<_> = fmts.iter().enumerate().filter(|(i, (_, _, w))| v[&format!("A{}", i + 1)].0 != *w).map(|(i, (x, f, w))| format!("{x} as {f}: {:?} ≠ {w:?}", v[&format!("A{}", i + 1)].0)).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

// --- workbook round trip -----------------------------------------------------------------------------

fn export(doc: &Doc) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let f = std::env::temp_dir().join(format!("calc-{}-{}.xlsx", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let c = Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2));
    crate::io::table::write_sheets_xlsx(&c, &f).unwrap();
    f
}
fn part(path: &std::path::Path, name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(&mut z.by_name(name).unwrap(), &mut s).unwrap();
    s
}

#[test]
fn excel_round_trip_keeps_formulas_values_and_formats() {
    let mut doc = small_plan();
    add(&mut doc, "Données ✓", &[("A1", "13/01/2026"), ("A2", "25%"), ("A3", "1234.5"), ("A4", "001"), ("A5", "TRUE"), ("A6", "=1/0"), ("A7", "=A1+7"), ("A8", "=PercentComplete"),
        ("A9", "=XLOOKUP(\"Design\",Tasks[Name],Tasks[Finish])"), ("A10", "=ProjectFinish"), ("A11", "$1,234.50"), ("A12", "=A3*2"), ("A13", "-1,234.5"), ("A14", "hello"), ("A15", "=\"x\"&A14"),
        ("A16", "=ROUND(PercentComplete,1)"), ("A17", "€12"), ("A18", "=COUNTIFS(Tasks[Finish],\"<=\"&DATE(2026,11,13))")]);
    set_fmt(&mut doc, 0, "A3", "money");
    set_fmt(&mut doc, 0, "A4", "@");
    set_fmt(&mut doc, 0, "A12", "#,##0.0");
    add(&mut doc, "Other", &[("A1", "='Données ✓'!A3*2")]);
    let before = shown_in(&doc, 0, "%d/%m/%Y");
    let back = crate::io::sheetbook::import(&export(&doc)).unwrap();
    let mut d2 = small_plan();
    d2.apply(Op::AddSheets { sheets: back.sheets, pictures: vec![], freeze: vec![] }, "%d/%m/%Y").unwrap();
    let after = shown_in(&d2, 0, "%d/%m/%Y");
    let bad: Vec<_> = before.iter().filter(|(a, b)| after.get(*a).map(|x| &x.0) != Some(&b.0)).map(|(a, b)| format!("{a}: {:?} → {:?}", b.0, after.get(a).map(|x| &x.0))).collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(shown_in(&d2, 1, "%d/%m/%Y")["A1"].0, "2469");
    assert_eq!(d2.p.sheets[0].cells["A9"], "=XLOOKUP(\"Design\",Tasks[Name],Tasks[Finish])", "formulas come back as written");
}

#[test]
fn imported_formulas_keep_reading_their_own_sheets_when_tabs_are_renamed() {
    // The plan has a Sheet1 already; a workbook brings its own Sheet1, Plan and Sheet1 (2).
    let mut doc = small_plan();
    add(&mut doc, "Sheet1", &[("A1", "999")]);
    let s = |n: &str, cells: &[(&str, &str)]| SheetData { name: n.into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() };
    let sheets = vec![s("Sheet1", &[("A1", "5")]), s("Plan", &[("A1", "7")]), s("Sheet1 (2)", &[("A1", "11")]), s("Sum", &[("A1", "=Sheet1!A1+Plan!A1"), ("A2", "='Sheet1 (2)'!A1"), ("A3", "=SUM(Sheet1:Plan!A1)")])];
    doc.apply(Op::AddSheets { sheets, pictures: vec![], freeze: vec![] }, "%d/%m/%Y").unwrap();
    let names: Vec<_> = doc.p.sheets.iter().map(|s| s.name.as_str()).collect();
    let k = names.iter().position(|n| *n == "Sum").unwrap();
    let v = shown_in(&doc, k, "%d/%m/%Y");
    assert_eq!((v["A1"].0.as_str(), v["A2"].0.as_str(), v["A3"].0.as_str()), ("12", "11", "12"), "tabs {names:?}, formulas {:?}", doc.p.sheets[k].cells);
}

#[test]
fn excel_export_gives_the_same_figures_when_the_workbook_recalculates() {
    let mut doc = small_plan();
    add(&mut doc, "S", &[("A1", "=SUM(Tasks[Total Slack])"), ("A2", "=XLOOKUP(\"Design\",Tasks[Task Name],Tasks[Finish])"), ("A3", "=PercentComplete"), ("A4", "=ROUND(PercentComplete,1)"), ("A5", "€12")]);
    let f = export(&doc);
    let x = part(&f, "xl/worksheets/sheet1.xml");
    // Column names as the task sheet calls them are written as the table's own columns.
    assert!(x.contains("SUM(Tasks[Total Float])") && x.contains("Tasks[Name],Tasks[Finish]"), "{x}");
    // The plan percentages carry their % sign; the euro amount its symbol.
    let st = part(&f, "xl/styles.xml");
    assert!(st.contains("formatCode=\"0&quot;%&quot;\"") && st.contains("formatCode=\"0.0&quot;%&quot;\"") && st.contains("€"), "{st}");
    // The plan's dates are whole days, as in the sheets.
    let wb = part(&f, "xl/workbook.xml");
    assert!(wb.contains(">46346<") && wb.contains(">46328<"), "ProjectFinish and ProjectStart as days: {wb}");
    let plan = part(&f, "xl/worksheets/sheet2.xml");
    assert!(plan.contains("<v>46339</v>") && !plan.contains("46339.7"), "Design's finish in the Plan table: {plan}");
}





#[test]
fn money_in_the_workbook_looks_as_in_the_plan() {
    // Euros with no decimals, written after the amount: 1,235 € in the plan is #,##0 "€" in the file.
    let mut doc = small_plan();
    doc.p.currency = Currency { code: "EUR".into(), symbol: "€".into(), decimals: 0, after: true };
    doc.p.tasks[0].fixed_cost = 1234.5;
    add(&mut doc, "M", &[("A1", "1234.5")]);
    set_fmt(&mut doc, 0, "A1", "money");
    assert_eq!(shown_in(&doc, 0, "%d/%m/%Y")["A1"].0, "1,235 €", "a half rounds up, as a spreadsheet shows it");
    let usd = Currency::default();
    assert_eq!((crate::fields::money(2.5, &Currency { decimals: 0, ..usd.clone() }), crate::fields::money(1.005, &usd), crate::fields::money(-0.125, &usd), crate::fields::money(0.004, &usd)), ("$3".into(), "$1.01".into(), "-$0.13".into(), "$0.00".into()));
    let st = part(&export(&doc), "xl/styles.xml");
    assert!(st.contains("formatCode=\"#,##0 &quot;€&quot;\"") && !st.contains("0.&quot;") && !st.contains("#,##0.\""), "{st}");
    let f = std::env::temp_dir().join(format!("calc-full-{}.xlsx", std::process::id()));
    crate::io::table::write_xlsx(&Calc::new(&doc.p, "%d/%m/%Y", d(2026, 11, 2)), &["name".to_string(), "cost".to_string()], &f).unwrap();
    let st = part(&f, "xl/styles.xml");
    assert!(!st.contains("#,##0.\"") && st.contains("#,##0 &quot;€&quot;"), "the task list's cost column: {st}");
}
