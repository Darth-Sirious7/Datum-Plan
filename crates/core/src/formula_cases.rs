use crate::{cal::at, doc::{Doc, Op}, fields::Calc, formula::{self, Ctx, F}, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }
struct C;
impl Ctx for C {
    fn field(&self, _: &str) -> Option<F> { Some(F::Null) }
    fn now(&self) -> i64 { 0 }
    fn work_between(&self, a: i64, b: i64) -> i64 { b - a }
    fn work_add(&self, a: i64, m: i64) -> i64 { a + m }
}
fn ev(s: &str) -> Result<F, String> { formula::eval(&formula::parse(s)?, &C) }
fn plan(n: usize) -> Project {
    let mut p = Project::new("T", d(2026, 11, 2));
    for i in 0..n { let t = p.new_task(&format!("T{}", i + 1), 0, 480); p.tasks.push(t); }
    p
}
fn op(doc: &mut Doc, o: Op) -> Result<(), String> { doc.apply(o, "%d-%b-%y") }
fn def(key: &str, alias: &str, f: &str) -> Op { Op::FieldDef { def: CustomDef { key: key.into(), alias: alias.into(), formula: f.into(), ..CustomDef::default() } } }

#[test] fn empty_string_literal() { assert_eq!(ev(r#""""#), Ok(F::Str(String::new()))); }
#[test] fn empty_string_compare() { assert_eq!(ev(r#"IIf("" = "", "a", "b")"#), Ok(F::Str("a".into()))); }
#[test] fn string_ending_in_escaped_quote() { assert_eq!(ev(r#""a""""#), Ok(F::Str("a\"".into()))); }
#[test] fn only_escaped_quote() { assert_eq!(ev(r#""""""#), Ok(F::Str("\"".into()))); }
#[test] fn instr_unicode() { assert_eq!(ev(r#"InStr("été", "t")"#), Ok(F::Num(2.0))); }
#[test] fn concat_decimal() { assert_eq!(ev(r#""x" & 2.5"#), Ok(F::Str("x2.5".into()))); }
#[test] fn concat_third() { assert_eq!(ev(r#"Len(CStr(1/8))"#), Ok(F::Num(5.0))); }
#[test] fn big_number_text() { assert_eq!(ev("CStr(10^20)"), Ok(F::Str("100000000000000000000".into()))); }
#[test] fn round_bankers() { assert_eq!(ev("Round(2.5)"), Ok(F::Num(2.0))); }
#[test] fn datediff_ww_sundays() { assert_eq!(ev("DateDiff(\"ww\", #10/31/2026#, #11/1/2026#)"), Ok(F::Num(1.0))); }
#[test] fn datediff_h_boundary() { assert_eq!(ev("DateDiff(\"h\", #10/31/2026# + 0.99, #11/1/2026#)"), Ok(F::Num(1.0))); }
#[test] fn dateadd_quarter() { assert_eq!(ev("DateAdd(\"q\", 1, #1/15/2026#)"), Ok(F::Date(at(d(2026, 4, 15))))); }
#[test] fn datediff_quarter() { assert_eq!(ev("DateDiff(\"q\", #1/15/2026#, #4/15/2026#)"), Ok(F::Num(1.0))); }
#[test] fn date_plus_huge_no_panic() { let r = std::panic::catch_unwind(|| ev("#1/1/2026# + 10^300")); assert!(r.is_ok()); }
#[test] fn dateadd_huge_no_panic() { let r = std::panic::catch_unwind(|| ev("DateAdd(\"d\", 10^300, #1/1/2026#)")); assert!(r.is_ok()); }
#[test] fn year_of_huge() { let r = std::panic::catch_unwind(|| ev("Year(10^300)")); assert!(r.is_ok()); }
#[test] fn neg_pow() { assert_eq!(ev("-2^2"), Ok(F::Num(-4.0))); }
#[test] fn mod_rounds() { assert_eq!(ev("5.5 Mod 2"), Ok(F::Num(0.0))); }
#[test] fn deep_parens() {
    let s = format!("{}1{}", "(".repeat(3000), ")".repeat(3000));
    let h = std::thread::Builder::new().stack_size(1 << 20).spawn(move || formula::parse(&s).map(|_| ())).unwrap();
    assert!(h.join().is_ok(), "stack overflow parsing 3000-deep parentheses on a 1 MB stack");
}

#[test]
fn self_referencing_custom_formula() {
    let _x = 0;
    let mut doc = Doc::new(plan(2));
    op(&mut doc, def("number1", "", "[Number1] + 1")).unwrap();
    // run in a thread so an overflow is observable
    let p = doc.p.clone();
    let h = std::thread::Builder::new().stack_size(8 << 20).spawn(move || { let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 2)); c.text(0, "c:number1") }).unwrap();
    assert_eq!(h.join().unwrap(), "#ERROR: the formula refers to itself");
}

#[test]
fn finish_rollup_shows_the_same_day() {
    let mut p = plan(2);
    let mut s = p.new_task("S", 0, 0);
    std::mem::swap(&mut s, &mut p.tasks[0]);
    p.tasks.insert(0, s);
    // tasks: [S(level 0), T1, T2] — make T1/T2 children
    let mut q = Project::new("T", d(2026, 11, 2));
    let s = q.new_task("S", 0, 0); q.tasks.push(s);
    for k in 0..2 { let t = q.new_task(&format!("T{k}"), 1, 480); q.tasks.push(t); }
    q.tasks[1].custom.insert("finish1".into(), serde_json::json!("2026-11-05"));
    q.tasks[2].custom.insert("finish1".into(), serde_json::json!("2026-11-04"));
    q.fields.push(CustomDef { key: "finish1".into(), alias: "F1".into(), rollup: Rollup::Max, ..CustomDef::default() });
    let c = Calc::new(&q, "%d-%b-%y", d(2026, 11, 2));
    assert_eq!(c.text(1, "c:finish1"), "05-Nov-26");
    assert_eq!(c.text(0, "c:finish1"), "05-Nov-26");
}

fn sheet_doc(cells: &[(&str, &str)]) -> Doc {
    let mut doc = Doc::new(plan(3));
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Data".into(), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Other".into(), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), fmt: None }).unwrap();
    doc
}
fn cell(doc: &Doc, k: usize, a: &str) -> String { doc.p.sheets[k].cells.get(a).cloned().unwrap_or_default() }
fn shown(doc: &Doc, k: usize, a: &str) -> String {
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    crate::sheet::sheet_view(&c, k).unwrap().cells.iter().find(|x| x.0 == a).map(|x| x.2.clone()).unwrap_or_default()
}

#[test]
fn sheet_insert_delete_shift() {
    let mut doc = sheet_doc(&[("A1", "1"), ("A2", "2"), ("A3", "3"), ("A4", "4"), ("A5", "5"), ("B1", "=SUM(A2:A5)"), ("B2", "=$A$5"), ("B3", "=SUM(A:A)"), ("C1", "=A3")]);
    op(&mut doc, Op::SheetCells { k: 1, cells: vec![("A1".into(), "=Data!A3".into()), ("A2".into(), "=A3".into())], fmt: None }).unwrap();
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 1, count: 1 }).unwrap(); // insert before row 2
    assert_eq!(cell(&doc, 0, "B1"), "=SUM(A3:A6)");
    assert_eq!(cell(&doc, 0, "B3"), "=$A$6");
    assert_eq!(cell(&doc, 0, "B4"), "=SUM(A:A)");
    assert_eq!(cell(&doc, 0, "C1"), "=A4");
    assert_eq!(cell(&doc, 1, "A1"), "=Data!A4");
    assert_eq!(cell(&doc, 1, "A2"), "=A3", "a reference on another sheet without a sheet name does not move");
    assert_eq!(shown(&doc, 0, "B1"), "14");
    // delete rows 3-4 (two of the summed rows)
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 2, count: -2 }).unwrap();
    assert_eq!(cell(&doc, 0, "B1"), "=SUM(A3:A4)");
    assert_eq!(cell(&doc, 0, "C1"), "=#REF!");
    assert_eq!(shown(&doc, 0, "B1"), "9");
}

#[test]
fn sheet_rename_with_space_quotes() {
    let mut doc = sheet_doc(&[("A1", "5")]);
    op(&mut doc, Op::SheetCells { k: 1, cells: vec![("A1".into(), "=Data!A1*2".into())], fmt: None }).unwrap();
    let mut s = doc.p.sheets[0].clone(); s.name = "My Data".into();
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) }).unwrap();
    assert_eq!(cell(&doc, 1, "A1"), "='My Data'!A1*2");
    assert_eq!(shown(&doc, 1, "A1"), "10");
    let mut s = doc.p.sheets[0].clone(); s.name = "Bob's".into();
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) }).unwrap();
    assert_eq!(shown(&doc, 1, "A1"), "10");
}

#[test]
fn sheet_spill_and_dash() {
    let doc = sheet_doc(&[("A1", "=SEQUENCE(3)"), ("B1", "=SUM(A1#)"), ("C1", "=A2")]);
    assert_eq!(shown(&doc, 0, "B1"), "6");
    assert_eq!(shown(&doc, 0, "C1"), "2");
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let r = crate::sheet::eval(&c, &[("=Data!A1:A3".into(), "".into())]);
    assert_eq!(r[0].iter().map(|x| x[0].0.clone()).collect::<Vec<_>>(), ["1", "2", "3"], "spilled cells in a sheet range");
}

#[test]
fn excel_basics() {
    let doc = sheet_doc(&[("A1", "=ROUND(2.5,0)"), ("A2", "=ROUND(-2.5,0)"), ("A3", "=1/0"), ("A4", "=IFERROR(1/0,\"x\")"), ("A5", "=SUMIF(B1:B3,\">1\")"), ("B1", "1"), ("B2", "2"), ("B3", "3"),
        ("A6", "=COUNTIF(B1:B3,\"<>2\")"), ("A7", "=TEXT(DATE(2026,1,5),\"yyyy-mm-dd\")"), ("A8", "=DATE(2026,1,5)"), ("A9", "=0.1+0.2"), ("A10", "=AND(TRUE,\"x\")"),
        ("A11", "=VLOOKUP(2,B1:C3,1,FALSE)"), ("A12", "=MID(\"héllo\",2,2)"), ("A13", "=LEN(\"日本\")"), ("A14", "=\"a\"&1.5"), ("A15", "=EOMONTH(DATE(2026,1,31),1)"), ("A16", "=10/3")]);
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    for x in &v.cells { eprintln!("{} {} => {}", x.0, x.1, x.2); }
}

#[test]
fn excel_column_reads_vba_field_that_reads_excel_field() {
    let mut doc = Doc::new(plan(2));
    op(&mut doc, def("number1", "Twice", "=[Duration]*2")).unwrap();
    op(&mut doc, def("number2", "Plus", "[Twice]+1")).unwrap();
    op(&mut doc, def("number3", "Again", "=[Plus]")).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    assert_eq!(c.text(0, "c:number1"), "2");
    assert_eq!(c.text(0, "c:number2"), "3");
    assert_eq!(c.text(0, "c:number3"), "3");
}


#[test]
fn vba_chain_perf() {
    let mut doc = Doc::new(plan(200));
    op(&mut doc, def("number1", "", "[Duration]")).unwrap();
    for k in 2..=16 { op(&mut doc, def(&format!("number{k}"), "", &format!("IIf([Number{0}] > 0, [Number{0}], 0)", k - 1))).unwrap(); }
    let t = std::time::Instant::now();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    for i in 0..200 { let _ = c.text(i, "c:number16"); }
    assert!(t.elapsed().as_secs() < 5, "200 rows, 16 chained IIf fields: {:?}", t.elapsed());
}

#[test] fn sgn_zero() { assert_eq!(ev("Sgn(0)"), Ok(F::Num(0.0))); }
#[test] fn instr_start_arg() { assert_eq!(ev(r#"InStr(3, "abcabc", "b")"#), Ok(F::Num(5.0))); }
#[test] fn date_text() { assert_eq!(ev(r#"CStr(#11/5/2026#)"#), Ok(F::Str("11/5/2026".into()))); }
#[test] fn max_empty() { assert!(ev("Max()").is_err() && ev("Min()").is_err(), "{:?} {:?}", ev("Max()"), ev("Min()")); }

// A date in text takes the plan's date format, as the grid shows it.
#[test]
fn date_text_in_plan_format() {
    let mut doc = Doc::new(plan(1));
    op(&mut doc, def("text1", "", r#""Starts " & [Start]"#)).unwrap();
    op(&mut doc, def("text2", "", "CStr([Start])")).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    assert_eq!((c.text(0, "c:text1").as_str(), c.text(0, "c:text2").as_str()), ("Starts 02-Nov-26", "02-Nov-26"));
}

#[test]
fn resource_formula_reads_resource_fields() {
    let mut doc = Doc::new(plan(1));
    doc.p.resources.push(Resource { uid: 1, name: "Fitter".into(), rate: 50.0, max: 1.0, ..Default::default() });
    let uid = doc.p.tasks[0].uid;
    op(&mut doc, Op::Set { uid, field: "res".into(), value: "Fitter".into() }).unwrap();
    op(&mut doc, def("rnumber1", "Day rate", "[Standard Rate]*8")).unwrap();
    op(&mut doc, def("rtext1", "", r#"IIf([Work] >= 8, "Busy at " & [Max Units] & "%, " & [Day rate] & " a day", "Free")"#)).unwrap();
    op(&mut doc, def("rcost1", "", "[Cost]*2")).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    assert_eq!(fields::res_text(&c, 0, "c:rnumber1"), "400");
    assert_eq!(fields::res_text(&c, 0, "c:rtext1"), "Busy at 100%, 400 a day");
    assert_eq!(fields::res_text(&c, 0, "c:rcost1"), fields::money(800.0, &doc.p.currency));
}

#[test]
fn formula_column_reads_whole_table_columns() {
    let mut doc = Doc::new(plan(2));
    (doc.p.tasks[0].fixed_cost, doc.p.tasks[1].fixed_cost) = (100.0, 300.0);
    doc.p.resources.push(Resource { uid: 1, name: "Fitter".into(), ..Default::default() });
    op(&mut doc, def("number1", "Share", "=[Cost]/SUM(Tasks[Cost])")).unwrap();
    op(&mut doc, def("number2", "People", "=COUNTA(Resources[Name])")).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    assert_eq!((c.text(0, "c:number1").as_str(), c.text(1, "c:number1").as_str()), ("0.25", "0.75"));
    assert_eq!(c.text(0, "c:number2"), "1");
}

#[test]
fn sheet_insert_refuses_to_push_cells_off_the_sheet() {
    let mut doc = sheet_doc(&[("A1", "1"), ("A1048570", "2"), ("XFA1", "3")]);
    assert!(op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 5, count: 10 }).is_err_and(|e| e.contains("last row")));
    assert!(op(&mut doc, Op::SheetInsert { k: 0, cols: true, at: 2, count: 5 }).is_err_and(|e| e.contains("last column")));
    assert_eq!(cell(&doc, 0, "A1048570"), "2");
    // Far past every cell: nothing moves, and the arithmetic does not overflow.
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: u32::MAX - 5, count: 10 }).unwrap();
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: u32::MAX - 5, count: -10 }).unwrap();
    assert_eq!(cell(&doc, 0, "A1048570"), "2");
}

#[test]
fn dashboard_tiles_follow_sheet_rename_and_insert() {
    let mut doc = sheet_doc(&[("E4", "1")]);
    let mut s = doc.p.sheets[0].clone(); s.name = "Weekly report".into();
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) }).unwrap();
    let tile = serde_json::json!({ "formula": "=HSTACK('Weekly report'!E4:E6,'Weekly report'!F4:F6)", "sub": "Weekly report!A1", "src": "='Weekly report'!A1:B2" });
    op(&mut doc, Op::Ui { key: "dashboards".into(), value: serde_json::json!([{ "name": "Main", "tiles": [tile] }]) }).unwrap();
    let mut s = doc.p.sheets[0].clone(); s.name = "Week".into();
    op(&mut doc, Op::Sheet { k: Some(0), sheet: Some(s) }).unwrap();
    let t = || doc.p.ui["dashboards"][0]["tiles"][0].clone();
    assert_eq!(t()["formula"], "=HSTACK(Week!E4:E6,Week!F4:F6)");
    assert_eq!((t()["src"].as_str(), t()["sub"].as_str()), (Some("=Week!A1:B2"), Some("Weekly report!A1")), "plain text stays as typed");
    op(&mut doc, Op::SheetInsert { k: 0, cols: false, at: 0, count: 1 }).unwrap();
    assert_eq!(doc.p.ui["dashboards"][0]["tiles"][0]["src"], "=Week!A2:B3");
}
#[test] fn nan_display() { eprintln!("{:?} {:?}", ev("CStr(-1^0.5)"), ev("CStr(10^400)")); }

fn try_on(stack: usize, s: String) -> bool {
    std::thread::Builder::new().stack_size(stack).spawn(move || { let e = formula::parse(&s); if let Ok(e) = e { let _ = formula::eval(&e, &C); } }).unwrap().join().is_ok()
}
#[test] #[ignore] fn depth_probe() {
    let n: usize = std::env::var("N").unwrap().parse().unwrap();
    let st: usize = std::env::var("ST").unwrap().parse().unwrap();
    let kind = std::env::var("K").unwrap();
    let s = match kind.as_str() { "p" => format!("{}1{}", "(".repeat(n), ")".repeat(n)), "s" => format!("{}1", "1+".repeat(n)), "n" => format!("{}1", "-".repeat(n)), _ => format!("{}1", "abs(".repeat(n)) + &")".repeat(n) };
    eprintln!("ok={}", try_on(st << 10, s));
}

#[test]
fn other_sheet_column_g_is_not_a_date() {
    let mut doc = sheet_doc(&[("G1", "5"), ("G2", "9"), ("K1", "1500")]);
    op(&mut doc, Op::SheetCells { k: 1, cells: vec![("A1".into(), "=MAX(Data!G1:G2)".into()), ("A2".into(), "=Data!K1".into())], fmt: None }).unwrap();
    assert_eq!(shown(&doc, 1, "A1"), "9");
    assert_eq!(shown(&doc, 1, "A2"), "1500");
}

#[test]
fn dash_whole_column_and_reversed() {
    let doc = sheet_doc(&[("A1", "x"), ("A2", "y"), ("B2", "2")]);
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let t = std::time::Instant::now();
    let r = crate::sheet::eval(&c, &[("=Data!B2:A1".into(), "".into()), ("=Data!A:A".into(), "".into()), ("=Data!A:B".into(), "".into())]);
    eprintln!("rev {:?}\nwhole {} rows in {:?}", r[0], r[1].len(), t.elapsed());
}
#[test] fn instr_more() {
    assert_eq!(ev(r#"InStr("abc", "c")"#), Ok(F::Num(3.0)));
    assert_eq!(ev(r#"InStr("abc", "")"#), Ok(F::Num(1.0)));
    assert_eq!(ev(r#"InStr("", "x")"#), Ok(F::Num(0.0)));
    assert_eq!(ev(r#"InStr(9, "abc", "c")"#), Ok(F::Num(0.0)));
    assert_eq!(ev(r#"InStr("ab", "abc")"#), Ok(F::Num(0.0)));
    assert_eq!(ev("7 \\ 2"), Ok(F::Num(3.0)));
    assert_eq!(ev("-7 Mod 3"), Ok(F::Num(-1.0)));
    assert!(ev("1 Mod 0.4").is_err());
}
#[test] fn negative_zero_display() {
    let mut p = plan(1);
    p.tasks[0].custom.insert("number1".into(), serde_json::json!(-0.001));
    let c = Calc::new(&p, "%d-%b-%y", d(2026, 11, 2));
    assert_eq!(c.text(0, "c:number1"), "0");
    assert_eq!(c.format(0, "pct", &F::Num(-0.2)), "0%");
}
#[test] fn excel_columns_2000_rows_time() {
    let mut doc = Doc::new(plan(2000));
    op(&mut doc, def("number1", "Twice", "=[Duration]*2")).unwrap();
    op(&mut doc, def("number2", "Plus", "[Twice]+1")).unwrap();
    let t = std::time::Instant::now();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    eprintln!("calc 2000 with excel+vba: {:?} {}", t.elapsed(), c.text(1999, "c:number2"));
}
