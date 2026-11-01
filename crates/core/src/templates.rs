//! Starter plans for File › New. Each row is `level|name|duration|predecessors` (row numbers).
//! `people` are the team (`name|group|rate per hour|max units|initials`, 8-hour days on the
//! Standard calendar), `assign` gives rows their people (`row|Name[50%],Other`), and `costs`
//! adds fixed costs (`row|amount`), so new plans show man-hours, costs and loading from the start.
use crate::{parse, Project};
use chrono::NaiveDate;

pub struct Template { pub id: &'static str, pub name: &'static str, pub desc: &'static str, pub color: &'static str, pub(crate) rows: &'static str, pub(crate) people: &'static str, pub(crate) assign: &'static str, pub(crate) costs: &'static str }

pub const TEMPLATES: &[Template] = &[
    Template { id: "engineering", name: "Engineering design", color: "#2F6FCF",
        desc: "Product design from requirements to release: concept, detailed design, prototype, testing and release to manufacturing.",
        rows: "0|Requirements\n1|Kickoff and project plan|2d\n1|Customer and design requirements|5d|2\n1|Requirements signed off|0|3\n\
0|Concept design\n1|Concept options and sketches|6d|4\n1|Concept evaluation and selection|3d|6\n1|Concept design review|0|7\n\
0|Detailed design\n1|3D CAD model|12d|8\n1|Calculations and analysis|8d|10SS+3d\n1|Drawings and bill of materials|6d|10\n1|Design FMEA|3d|11\n1|Critical design review|0|12,13\n\
0|Prototype\n1|Order prototype parts|3d|14\n1|Parts lead time|15d|16\n1|Build prototypes|5d|17\n\
0|Testing and validation\n1|Test plan|3d|14\n1|Functional testing|6d|18,20\n1|Environmental and durability tests|10d|21SS+2d\n1|Design updates from testing|5d|21\n1|Validation report|3d|22,23\n\
0|Release\n1|Final drawing release|2d|24\n1|Manufacturing handover|2d|26\n1|Released to manufacturing|0|27",
        people: "Project Manager|Management|95|1|PM\nDesign Engineer|Engineering|75|2|DE\nAnalysis Engineer|Engineering|80|1|AE\nCAD Designer|Engineering|55|1|CD\nTest Technician|Testing|50|2|TT\nPurchasing Officer|Procurement|45|1|PO",
        assign: "2|Project Manager,Design Engineer\n3|Design Engineer,Project Manager[50%]\n6|Design Engineer[200%]\n7|Design Engineer,Analysis Engineer\n10|CAD Designer,Design Engineer\n11|Analysis Engineer\n12|CAD Designer\n13|Design Engineer,Analysis Engineer[50%]\n16|Purchasing Officer\n18|Test Technician[200%],Design Engineer[50%]\n20|Test Technician,Design Engineer[50%]\n21|Test Technician[200%]\n22|Test Technician[200%],Analysis Engineer[50%]\n23|Design Engineer,CAD Designer\n24|Design Engineer,Project Manager[25%]\n26|CAD Designer,Design Engineer[50%]\n27|Project Manager,Design Engineer",
        costs: "16|18000\n17|4500\n22|12000" },
    Template { id: "epc", name: "EPC project", color: "#C2611F",
        desc: "Engineering, procurement and construction of a plant unit, through commissioning and handover, with long-lead equipment.",
        rows: "0|Engineering\n1|Basic engineering package review|10d\n1|Process design and P&IDs|20d|2\n1|Mechanical and piping design|30d|3SS+10d\n1|Electrical and instrumentation design|25d|3SS+15d\n1|Issued for construction drawings|0|4,5\n\
0|Procurement\n1|Long-lead equipment orders|10d|3\n1|Long-lead equipment manufacture|80d|8\n1|Bulk materials purchase|15d|4\n1|Equipment delivered to site|0|9\n\
0|Construction\n1|Site mobilisation|5d|6\n1|Civil works and foundations|30d|13\n1|Equipment installation|20d|14,11\n1|Piping installation|35d|15SS+10d,10\n1|Electrical and instrumentation installation|30d|16SS+10d\n1|Mechanical completion|0|16,17\n\
0|Commissioning and handover\n1|Pre-commissioning checks|10d|18\n1|Commissioning|15d|20\n1|Performance test run|5d|21\n1|Handover to client|0|22",
        people: "Project Manager|Management|120|1|PM\nProcess Engineer|Engineering|95|2|PE\nPiping Engineer|Engineering|90|3|PIE\nE&I Engineer|Engineering|90|2|EIE\nProcurement Lead|Procurement|80|1|PL\nConstruction Manager|Construction|110|1|CM\nCivil Crew|Construction|55|8|CC\nPipefitter|Construction|60|10|PF\nElectrician|Construction|58|6|EL\nCommissioning Engineer|Commissioning|100|3|CE",
        assign: "2|Project Manager,Process Engineer[200%]\n3|Process Engineer[200%]\n4|Piping Engineer[300%]\n5|E&I Engineer[200%]\n8|Procurement Lead,Process Engineer[50%]\n10|Procurement Lead\n13|Construction Manager,Civil Crew[200%]\n14|Construction Manager,Civil Crew[800%]\n15|Construction Manager,Pipefitter[400%]\n16|Construction Manager[50%],Pipefitter[1000%]\n17|Electrician[600%],E&I Engineer[50%]\n20|Commissioning Engineer[200%],Process Engineer\n21|Commissioning Engineer[300%],Process Engineer\n22|Commissioning Engineer[200%],Project Manager[50%]",
        costs: "9|2400000\n10|350000\n14|180000" },
    Template { id: "software", name: "Software development", color: "#2E9D6B",
        desc: "A product release in sprints: discovery, design, three build sprints, testing, launch and support.",
        rows: "0|Discovery\n1|Kickoff|1d\n1|Requirements and user stories|5d|2\n1|Architecture and technical design|5d|3\n1|Discovery complete|0|4\n\
0|Design\n1|UX wireframes|5d|3\n1|UI design|5d|7\n1|Design sign-off|0|8,4\n\
0|Build\n1|Sprint 1|10d|9\n1|Sprint 2|10d|11\n1|Sprint 3|10d|12\n1|Feature complete|0|13\n\
0|Test and launch\n1|System and integration testing|8d|14\n1|User acceptance testing|5d|16\n1|Release preparation|3d|17SS+2d\n1|Go live|0|17,18\n\
0|Support\n1|Hypercare|10d|19\n1|Project close|0|21",
        people: "Product Owner|Management|85|1|PO\nTech Lead|Engineering|95|1|TL\nDeveloper|Engineering|75|4|DEV\nUX Designer|Design|70|1|UX\nQA Engineer|Quality|60|2|QA",
        assign: "2|Product Owner,Tech Lead\n3|Product Owner,UX Designer[50%]\n4|Tech Lead,Developer\n7|UX Designer\n8|UX Designer\n11|Developer[400%],Tech Lead,QA Engineer[50%]\n12|Developer[400%],Tech Lead,QA Engineer[50%]\n13|Developer[400%],Tech Lead,QA Engineer\n16|QA Engineer[200%],Developer\n17|QA Engineer,Product Owner[50%]\n18|Tech Lead,Developer\n21|Developer[200%],QA Engineer[50%]",
        costs: "18|6000" },
    Template { id: "building", name: "Building construction", color: "#8A6D1F",
        desc: "A commercial building from permits to handover: foundations, frame, envelope, services, fit-out and inspections.",
        rows: "0|Pre-construction\n1|Design approval and permits|20d\n1|Contractor tendering|15d|2SS+10d\n1|Contract award|0|2,3\n\
0|Substructure\n1|Site setup|5d|4\n1|Excavation|8d|6\n1|Foundations|12d|7\n1|Ground floor slab|6d|8\n\
0|Superstructure\n1|Structural frame|25d|9\n1|Floors and roof structure|15d|11SS+10d\n1|Structure complete|0|11,12\n\
0|Envelope\n1|External walls|20d|13\n1|Roofing|10d|12\n1|Windows and doors|10d|15SS+10d\n1|Building watertight|0|16,17\n\
0|Services and fit-out\n1|Mechanical and plumbing|25d|18\n1|Electrical|25d|18\n1|Internal finishes|30d|20SS+10d,21SS+10d\n1|External works|15d|18\n\
0|Handover\n1|Inspections and snagging|10d|22,23\n1|Certificates and as-built documents|5d|25\n1|Practical completion|0|26",
        people: "Project Manager|Management|110|1|PM\nSite Engineer|Engineering|85|2|SE\nLabourer|Construction|40|10|LAB\nConcrete Crew|Construction|55|8|CON\nSteel Erector|Construction|65|6|STE\nCarpenter|Construction|55|4|CAR\nPlumber|Services|60|3|PLU\nElectrician|Services|60|4|ELE\nFinisher|Fit-out|50|6|FIN",
        assign: "2|Project Manager\n3|Project Manager[50%],Site Engineer\n6|Site Engineer,Labourer[400%]\n7|Labourer[600%],Site Engineer[50%]\n8|Concrete Crew[600%],Site Engineer\n9|Concrete Crew[600%]\n11|Steel Erector[600%],Site Engineer\n12|Steel Erector[400%],Carpenter[200%]\n15|Labourer[400%],Carpenter[200%]\n16|Carpenter[300%]\n17|Carpenter[200%]\n20|Plumber[300%]\n21|Electrician[400%]\n22|Finisher[600%],Carpenter[200%]\n23|Labourer[400%]\n25|Site Engineer,Project Manager[50%]\n26|Project Manager[50%],Site Engineer",
        costs: "8|85000\n11|240000\n17|60000" },
    Template { id: "research", name: "Research project", color: "#7A4FC4",
        desc: "A funded study: proposal and approvals, literature review, data collection, analysis and publication.",
        rows: "0|Proposal\n1|Literature scan|10d\n1|Write proposal|10d|2\n1|Ethics and funding approval|20d|3\n1|Approved|0|4\n\
0|Literature review\n1|Systematic search|10d|5\n1|Review and synthesis|15d|7\n\
0|Data collection\n1|Instrument design|10d|5\n1|Pilot study|5d|10\n1|Main data collection|40d|11\n1|Data collected|0|12\n\
0|Analysis\n1|Data cleaning|8d|13\n1|Statistical analysis|15d|15\n1|Interpretation|10d|16,8\n\
0|Publication\n1|Draft paper|15d|17\n1|Internal review|5d|19\n1|Submit for publication|0|20",
        people: "Principal Investigator|Research|90|1|PI\nResearch Assistant|Research|40|2|RA\nStatistician|Analysis|75|1|ST\nField Officer|Field|35|4|FO",
        assign: "2|Principal Investigator,Research Assistant\n3|Principal Investigator\n4|Principal Investigator[25%]\n7|Research Assistant[200%]\n8|Principal Investigator[50%],Research Assistant\n10|Principal Investigator[50%],Research Assistant\n11|Field Officer[200%],Research Assistant\n12|Field Officer[400%],Research Assistant\n15|Research Assistant,Statistician[50%]\n16|Statistician\n17|Principal Investigator,Statistician[50%]\n19|Principal Investigator,Research Assistant[50%]\n20|Principal Investigator[25%]",
        costs: "12|15000" },
];

pub fn build(t: &Template, start: NaiveDate) -> Project {
    let mut p = Project::new(t.name, start);
    let rows: Vec<Vec<&str>> = t.rows.lines().map(|l| l.split('|').collect()).collect();
    for r in &rows {
        let task = p.new_task(r[1], r[0].parse().unwrap_or(0), r.get(2).map_or(0, |d| parse::dur(d, 480).map_or(0, |x| x.0)));
        p.tasks.push(task);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (task, r) in p.tasks.iter_mut().zip(&rows) {
        task.preds = parse::preds(r.get(3).unwrap_or(&""), &ids, 480).expect("template links are valid");
    }
    for l in t.people.lines() {
        let f: Vec<&str> = l.split('|').collect();
        let uid = p.resource(f[0]);
        let r = p.res_idx(uid).map(|k| &mut p.resources[k]).expect("just added");
        r.group = f[1].into();
        r.rate = f[2].parse().unwrap_or(0.0);
        r.max = f[3].parse().unwrap_or(1.0);
        r.initials = f[4].into();
    }
    for l in t.assign.lines() {
        let (row, spec) = l.split_once('|').expect("row|people");
        let i: usize = row.parse::<usize>().expect("row number") - 1;
        p.tasks[i].res = parse::res(spec, &mut p).expect("template people are valid");
    }
    for l in t.costs.lines().filter(|l| !l.is_empty()) {
        let (row, amt) = l.split_once('|').expect("row|amount");
        p.tasks[row.parse::<usize>().expect("row number") - 1].fixed_cost = amt.parse().unwrap_or(0.0);
    }
    // Show what the team costs and how many man-hours each task takes.
    p.titles.insert("work".into(), "Man-hours".into());
    p.columns = vec![("mode".into(), 34), ("name".into(), 260), ("dur".into(), 74), ("start".into(), 92), ("finish".into(), 92), ("preds".into(), 90), ("res".into(), 170), ("work".into(), 84), ("cost".into(), 100)];
    p
}

/// The four plans of the linked example: a main schedule and three subplans, each its own file.
const LINKED: [(&str, Template); 4] = [
    ("Main schedule.dplan", Template { id: "x-main", name: "Main schedule", color: "#14325E", desc: "",
        rows: "0|Project milestones\n1|Contract award|0\n1|Kick-off meeting|1d|2\n1|Site permits granted|0|2FS+30d\n1|Engineering complete|0\n1|Procurement complete|0\n\
1|Mechanical completion|0\n1|Ready for start-up|0\n1|Performance test and acceptance|5d|8\n1|Handover to operations|0|9\n\
0|Project controls\n1|Baseline schedule approved|5d|3\n1|HSE plan approved|5d|3\n1|Controls in place|0|12,13",
        people: "Project Director|Management|150|1|PD\nProject Controls Lead|Controls|95|1|PCL",
        assign: "3|Project Director,Project Controls Lead\n9|Project Director\n12|Project Controls Lead\n13|Project Director[50%]", costs: "" }),
    ("Engineering.dplan", Template { id: "x-eng", name: "Engineering", color: "#2F6FCF", desc: "",
        rows: "0|Basic engineering\n1|Kick-off and design basis|5d\n1|Process simulation and heat and mass balance|15d|2\n1|P&IDs issued for review|10d|3\n1|HAZOP study|5d|4\n1|Design freeze|0|5\n\
0|Detailed engineering\n1|Compressor package datasheets|10d|4\n1|Mechanical datasheets issued|0|8\n1|Plot plan and layout|10d|6\n1|Civil and structural design|25d|10\n1|Civil drawings IFC|0|11\n\
1|Piping design and 3D model|35d|10\n1|Piping isometrics IFC|10d|13\n1|Electrical and instrumentation design|30d|6SS+10d\n1|E&I drawings IFC|0|15\n\
0|Close-out\n1|Vendor document review|20d|9FS+30d\n1|Final documentation issued|10d|14,16,18",
        people: "Lead Process Engineer|Process|95|1|LPE\nMechanical Engineer|Mechanical|85|2|ME\nPiping Designer|Piping|60|3|PD\nCivil Engineer|Civil|80|1|CE\nE&I Engineer|Electrical|85|2|EIE",
        assign: "2|Lead Process Engineer,Mechanical Engineer\n3|Lead Process Engineer\n4|Lead Process Engineer,Piping Designer\n5|Lead Process Engineer,Mechanical Engineer,E&I Engineer\n8|Mechanical Engineer[200%]\n\
10|Piping Designer,Civil Engineer\n11|Civil Engineer\n13|Piping Designer[300%]\n14|Piping Designer[200%]\n15|E&I Engineer[200%]\n18|Mechanical Engineer,E&I Engineer\n19|Lead Process Engineer,Piping Designer", costs: "" }),
    ("Procurement.dplan", Template { id: "x-proc", name: "Procurement", color: "#C2611F", desc: "",
        rows: "0|Compressor package (long lead)\n1|Compressor package RFQ|10d\n1|Bid evaluation and technical clarifications|15d|2\n1|Compressor package PO placed|0|3\n\
1|Manufacture and factory acceptance test|120d|4\n1|Shipping and customs clearance|25d|5\n1|Compressor package delivered to site|0|6\n\
0|Electrical equipment\n1|Switchgear and transformer purchase|15d\n1|Manufacture and delivery|70d|9\n1|Electrical equipment delivered|0|10\n\
0|Bulk materials\n1|Piping bulk materials purchase|15d\n1|Bulk materials delivery|40d|13\n1|All equipment on site|0|7,11,14",
        people: "Procurement Lead|Procurement|80|1|PRL\nBuyer|Procurement|60|2|BUY\nExpediter|Procurement|55|1|EXP",
        assign: "2|Procurement Lead,Buyer\n3|Procurement Lead,Buyer\n5|Expediter[25%]\n6|Expediter[50%]\n9|Buyer\n10|Expediter[25%]\n13|Buyer\n14|Expediter[25%]",
        costs: "5|4800000\n6|180000\n10|650000\n14|900000" }),
    ("Construction.dplan", Template { id: "x-con", name: "Construction", color: "#2E9D6B", desc: "",
        rows: "0|Site preparation\n1|Site mobilisation|10d\n1|Temporary facilities|5d|2\n\
0|Civil works\n1|Foundations and civil works|40d|3\n1|Compressor foundation cured|0|5FS+10d\n\
0|Mechanical installation\n1|Set compressor package on foundation|5d|6\n1|Piping installation|45d|8SS+3d\n1|Hydrotest and reinstatement|10d|9\n\
0|Electrical and instrumentation\n1|Electrical installation|30d|5\n1|Instrumentation and cabling|25d|12SS+10d\n1|Mechanical completion|0|10,13\n\
0|Commissioning\n1|Pre-commissioning|15d|14\n1|Commissioning|20d|16\n1|Commissioning complete|0|17",
        people: "Construction Manager|Management|110|1|CM\nCivil Crew|Civil|45|8|CC\nMechanical Crew|Mechanical|55|10|MC\nE&I Crew|Electrical|55|6|EC\nCommissioning Engineer|Commissioning|95|2|COM",
        assign: "2|Construction Manager,Civil Crew[400%]\n3|Civil Crew[400%]\n5|Civil Crew[800%]\n8|Mechanical Crew[600%]\n9|Mechanical Crew[1000%]\n10|Mechanical Crew[400%]\n\
12|E&I Crew[600%]\n13|E&I Crew[400%]\n16|Commissioning Engineer[200%],Mechanical Crew[200%]\n17|Commissioning Engineer[200%],E&I Crew[200%]",
        costs: "3|85000" }),
];
/// Links between the plans: (plan, row, from plan, from row, type, lag in days).
const LINKS: [(usize, usize, usize, usize, crate::model::LinkType, i64); 13] = {
    use crate::model::LinkType::*;
    [(1, 2, 0, 2, FS, 0), (0, 5, 1, 19, FS, 0), (2, 2, 1, 9, FS, 0), (2, 9, 1, 16, FS, 0), (2, 13, 1, 14, FS, 0), (0, 6, 2, 15, FS, 0),
     (3, 2, 0, 4, FS, 0), (3, 5, 1, 12, FS, 0), (3, 8, 2, 7, FS, 5), (3, 9, 2, 14, FS, 0), (3, 12, 2, 11, FS, 0), (0, 7, 3, 14, FS, 0), (0, 8, 3, 18, FS, 0)]
};

/// The linked example: a main schedule and three subplans (engineering, procurement,
/// construction) joined by 13 links between the plans, as (file name, plan). Their links are
/// already up to date. Engineering and construction use resource calendars: an engineer's
/// leave, and a six-day site week with a crew holiday.
pub fn linked_example(start: NaiveDate) -> Vec<(String, Project)> {
    use crate::model::{Calendar, ExtLink, Exception, STD_HOURS};
    use chrono::Datelike;
    let mut plans: Vec<(String, Project)> = LINKED.iter().map(|(f, t)| (f.to_string(), build(t, start))).collect();
    for &(to, row, from, frow, kind, lag) in &LINKS {
        let uid = plans[from].1.tasks[frow - 1].uid;
        let file = LINKED[from].0.to_string();
        plans[to].1.tasks[row - 1].ext.push(ExtLink { file, uid, kind, lag: lag * 480, plan: String::new(), code: String::new(), name: String::new(), start: 0, finish: 0 });
    }
    let day = |n: u64| start.checked_add_days(chrono::Days::new(n)).unwrap_or(start);
    let countries: [&[&str]; 4] = [&["GB", "AE"], &["DE", "DE", "IN", "IN", "GB"], &["AE", "AE", "CN"], &["GB", "IN", "PH", "PK", "DE"]];
    for ((_, p), cs) in plans.iter_mut().zip(countries) {
        p.resources.iter_mut().zip(cs).for_each(|(r, c)| r.country = c.to_string());
        let at = p.columns.iter().position(|c| c.0 == "preds").unwrap_or(5);
        p.columns.insert(at, ("ext".into(), 260));
    }
    // Engineering: the lead process engineer is on leave for two weeks during the simulation.
    let eng = &mut plans[1].1;
    eng.opts.res_cals = true;
    eng.resources[0].exceptions.push(Exception { name: "Annual leave".into(), from: day(14), to: day(25), hours: vec![] });
    // Construction: crews work a six-day week, and the mechanical crew has a four-day holiday break.
    let con = &mut plans[3].1;
    con.opts.res_cals = true;
    let mut six = Calendar::standard("Site 6-day week");
    six.week[5] = STD_HOURS.to_vec();
    con.calendars.push(six);
    for r in con.resources.iter_mut().filter(|r| r.name.ends_with("Crew")) { r.cal = Some("Site 6-day week".into()) }
    con.resources[2].exceptions.push(Exception { name: "Holiday break".into(), from: day(250), to: day(253), hours: vec![] });
    // Every calendar closes for Christmas and New Year, as most sites do.
    for (_, p) in &mut plans {
        for c in &mut p.calendars {
            for y in start.year()..=start.year() + 2 {
                let (Some(from), Some(to)) = (NaiveDate::from_ymd_opt(y, 12, 24), NaiveDate::from_ymd_opt(y + 1, 1, 1)) else { continue };
                c.exceptions.push(Exception { name: format!("Christmas and New Year {y}"), from, to, hours: vec![] });
            }
        }
    }
    // One project: the main schedule is the master of the three subplans.
    plans[0].1.subplans = LINKED[1..].iter().zip(&plans[1..]).map(|((f, _), (_, p))| crate::model::SubPlan { file: f.to_string(), name: p.name.clone(), level: 0 }).collect();
    for (_, p) in &mut plans[1..] { p.master = LINKED[0].0.to_string() }
    let _ = crate::links::update_all(&mut plans);
    plans
}
