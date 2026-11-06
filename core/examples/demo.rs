//! Writes synthetic demo data for the screenshot harness (app/harness/).
//! Every outlet, story, person and book text here is invented; the web
//! addresses use the reserved .example domain. The data still goes through the
//! real code: the edition builder, the bias meter and the archive search.
//!
//!   cargo run -q -p granthalaya-core --example demo > app/harness/demo.json

use std::collections::HashMap;

use chrono::{DateTime, Duration, TimeZone, Utc};
use granthalaya_core::bias::{self, Link, OutletFacts};
use granthalaya_core::db::Store;
use granthalaya_core::edition::{self, Edition, Prefs};
use granthalaya_core::feeds::{Story, story_id};
use granthalaya_core::{comfort, sources, topics};
use serde_json::{Value, json};

struct Outlet {
    id: &'static str,
    name: &'static str,
    country: &'static str,
    funding: &'static str,
    owner: Option<&'static str>,
}

const OUTLETS: &[Outlet] = &[
    Outlet {
        id: "lantern",
        name: "The Lantern Gazette",
        country: "United Kingdom",
        funding: "commercial",
        owner: Some("Lantern Media Group"),
    },
    Outlet { id: "meridian", name: "Meridian Courier", country: "Canada", funding: "public", owner: None },
    Outlet { id: "harbour", name: "Harbourline", country: "Singapore", funding: "commercial", owner: None },
    Outlet { id: "northwind", name: "Northwind Times", country: "United States", funding: "nonprofit", owner: None },
    Outlet { id: "quill", name: "The Quill Report", country: "India", funding: "commercial", owner: Some("Lantern Media Group") },
    Outlet { id: "copper", name: "Copperfield Daily", country: "Australia", funding: "commercial", owner: None },
    Outlet { id: "saltmarsh", name: "Saltmarsh Herald", country: "Ireland", funding: "commercial", owner: None },
    Outlet { id: "tidewater", name: "Tidewater Radio", country: "Germany", funding: "state", owner: None },
];

/// (day before the newest edition, outlet, section, title, summary)
const STORIES: &[(i64, &str, &str, &str, &str)] = &[
    (
        0,
        "meridian",
        "world",
        "Coastal city closes new flood barrier as storm surge nears",
        "The barrier's twelve steel gates were lowered overnight, city engineers said, the first full closure since the system was finished last spring.",
    ),
    (
        0,
        "harbour",
        "world",
        "Storm surge: new flood barrier closed for first time",
        "Engineers lowered all twelve gates of the coastal city's flood barrier on Wednesday night, according to the regional water authority.",
    ),
    (
        0,
        "saltmarsh",
        "world",
        "Flood barrier gates lowered as storm surge approaches coast",
        "The city's water authority confirmed the closure, saying tide gauges showed water two metres above normal.",
    ),
    (
        0,
        "lantern",
        "world",
        "Officials slammed over flood barrier delays as storm surge hits",
        "Critics blasted the shocking cost of the flood barrier, which many say came too late for low-lying streets.",
    ),
    (
        0,
        "northwind",
        "science",
        "Small telescope team finds comet heading toward inner solar system",
        "Three amateur astronomers spotted the faint comet in images taken last week, the regional observatory said in a statement.",
    ),
    (
        0,
        "tidewater",
        "science",
        "Amateur astronomers' comet confirmed by observatory",
        "The observatory said follow-up images confirmed the object, which should be visible with binoculars in March.",
    ),
    (
        0,
        "copper",
        "business",
        "Rail freight operator orders forty battery locomotives",
        "The operator said the locomotives would replace diesel engines on its shortest routes by 2029, according to its annual report.",
    ),
    (
        0,
        "meridian",
        "business",
        "Freight operator to switch short routes to battery locomotives",
        "The forty-locomotive order is the largest of its kind in the country, the transport ministry said.",
    ),
    (
        0,
        "quill",
        "business",
        "Battery locomotive order signals shift for rail freight",
        "Analysts said the order of forty locomotives could lower freight costs on short routes.",
    ),
    (
        0,
        "harbour",
        "health",
        "Clinics extend evening hours to shorten waiting lists",
        "Twenty community clinics will stay open until nine in the evening from next month, the health board said.",
    ),
    (
        0,
        "saltmarsh",
        "culture",
        "Village library reopens after volunteers rebuild its reading room",
        "Volunteers spent eight months restoring the building, the parish council said, and lending begins on Saturday.",
    ),
    (
        0,
        "northwind",
        "investigations",
        "Records show water testing at schools lapsed for two years",
        "Testing logs obtained by Northwind Times show that forty schools missed required checks, according to the documents.",
    ),
    (
        0,
        "copper",
        "sport",
        "Underdog club reaches cup final after penalty shootout",
        "The second-tier side won five to four on penalties, the league confirmed, setting up a final against the champions.",
    ),
    (
        0,
        "tidewater",
        "technology",
        "Open-source map project adds offline walking routes",
        "The volunteer project said the routes cover forty countries and work without a data connection.",
    ),
    (
        0,
        "lantern",
        "ideas",
        "Opinion: our towns need slower streets, not wider roads",
        "In this column, a planner argues that narrower streets would make town centres safer and busier.",
    ),
    (
        1,
        "meridian",
        "world",
        "Water authority warns of high tides along the coast this week",
        "The authority said the new flood barrier could close for the first time if a storm arrives as forecast.",
    ),
    (
        1,
        "harbour",
        "science",
        "Ocean buoy network records warmest autumn water in a decade",
        "Readings from sixty buoys show surface water a degree warmer than the ten-year average, researchers said.",
    ),
    (
        1,
        "copper",
        "business",
        "Port expands night shifts as container traffic rises",
        "The port said it would add two night shifts from December, according to a statement.",
    ),
    (
        2,
        "saltmarsh",
        "world",
        "Flood barrier passes final inspection before storm season",
        "Inspectors signed off the barrier's twelve gates on Monday, the city said.",
    ),
    (
        2,
        "northwind",
        "culture",
        "Community choir records album in disused train station",
        "The choir of eighty singers recorded twelve songs over two nights, its director said.",
    ),
];

/// Body of the story the screenshots open: the lead flood-barrier telling.
const BODY: &[&str] = &[
    "The coastal city's new flood barrier was closed in full for the first time on Wednesday night, as a storm pushed water two metres above the normal tide.",
    "All twelve steel gates were lowered between nine and eleven in the evening, according to the regional water authority. Engineers said the closure went to plan.",
    "\"The gates held, and the water behind them stayed at a normal level,\" said Ana Ruiz, the authority's chief engineer. \"That is exactly what the barrier was built to do.\"",
    "The barrier took six years to build. Its cost rose by a fifth during construction, the city's audit office reported last year.",
    "Residents of the low-lying harbour district were asked to stay indoors until the morning. The authority said the gates would reopen at low tide on Thursday.",
];

const BOOK_TITLE: &str = "The Lighthouse Almanac";
const BOOK_AUTHOR: &str = "Tamsin Vale";

/// A short invented book in FictionBook 2 (plain XML), so the harness can open
/// it in the real reader without a zip library.
fn fb2() -> String {
    let chapters: &[(&str, &[&str])] = &[
        (
            "I. The Keeper",
            &[
                "The lighthouse stood at the end of a long spit of shingle, and on clear evenings its shadow reached almost to the village. Mara had kept it for eleven years, and in that time she had learned that the sea never repeated itself.",
                "Each morning she wrote three lines in the almanac: the wind, the colour of the water, and one thing she had not expected. The third line was the hardest. Some days she sat with the pen above the page until her tea went cold.",
                "Patience, her uncle had told her when he handed over the keys, is not waiting. It is paying attention while you wait. She had thought it a pretty saying then. Now she knew it was simply a description of the work.",
                "The lamp was lit at dusk and put out at dawn. Between those hours the lighthouse belonged to the ships, and the ships did not know her name. That suited her. A good light, she thought, should be noticed only when it is missing.",
            ],
        ),
        (
            "II. The Storm Book",
            &[
                "In the third winter a storm came that the almanac had no words for. The barometer fell so fast that Mara tapped the glass, thinking it stuck. Outside, the gulls had gone quiet and the water had turned the grey of old pewter.",
                "She trimmed the wick, filled the reservoir and climbed the hundred and twelve steps twice more than she needed to, because climbing was something to do with her hands. At midnight the first wave broke over the gallery rail.",
                "By morning the shingle had moved. The path to the village was gone, and a new beach lay where the boathouse had been. She opened the almanac and wrote, under the third line, a single word: everything.",
            ],
        ),
        (
            "III. Small Repairs",
            &[
                "Spring came slowly. The village sent a cart along the new beach with timber and nails, and two boys who wanted to see the lamp. Mara let them polish the brass, which they did with more enthusiasm than care.",
                "Repairs, she found, were mostly small. A hinge here, a pane of glass there, a stair tread that had worked loose. None of it was dramatic, and all of it mattered. The light did not care how the tower looked; it cared whether the tower stood.",
                "On the first warm evening she carried a chair onto the gallery and watched the fishing boats come home, one after another, each one turning a little as it found her beam. She did not write anything in the almanac that night. Some days, she decided, the third line could be left blank.",
            ],
        ),
    ];
    let mut body = String::new();
    for (title, paras) in chapters {
        body.push_str(&format!("<section><title><p>{title}</p></title>"));
        for p in *paras {
            body.push_str(&format!("<p>{p}</p>"));
        }
        body.push_str("</section>");
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0"><description><title-info><genre>prose</genre><author><first-name>Tamsin</first-name><last-name>Vale</last-name></author><book-title>{BOOK_TITLE}</book-title><lang>en</lang></title-info></description><body><title><p>{BOOK_TITLE}</p></title>{body}</body></FictionBook>"#
    )
}

fn facts() -> HashMap<String, OutletFacts> {
    OUTLETS
        .iter()
        .map(|o| {
            let f = OutletFacts {
                id: o.id.into(),
                name: o.name.into(),
                country: o.country.into(),
                funding: o.funding.into(),
                funding_text: sources::funding_text(o.funding).into(),
                owners: o.owner.map(|n| vec![Link { label: n.into(), url: None }]).unwrap_or_default(),
                funders: vec![],
                wikidata: None,
            };
            (o.id.to_string(), f)
        })
        .collect()
}

fn stories(newest: DateTime<Utc>, day: i64) -> Vec<Story> {
    STORIES
        .iter()
        .enumerate()
        .filter(|(_, s)| s.0 == day)
        .map(|(i, &(_, outlet, section, title, summary))| {
            let o = OUTLETS.iter().find(|o| o.id == outlet).expect("outlet");
            let url = format!("https://{outlet}.example/{section}/{i}");
            Story {
                id: story_id(&url),
                outlet_id: outlet.into(),
                outlet_name: o.name.into(),
                section: section.into(),
                title: title.into(),
                summary: summary.into(),
                url,
                published: Some(newest - Duration::days(day) - Duration::minutes(20 * i as i64)),
                image: None,
                topics: topics::tag(title, summary).into_iter().map(String::from).collect(),
                paywall: false,
            }
        })
        .collect()
}

fn book(id: &str, title: &str, author: &str, subjects: &[&str], summary: &str) -> Value {
    json!({ "id": id, "title": title, "authors": [author], "subjects": subjects, "summary": summary,
            "cover": null, "epub_url": null, "source": "Imported", "popularity": 0 })
}

fn main() -> anyhow::Result<()> {
    let newest = Utc.with_ymd_and_hms(2026, 11, 5, 7, 30, 0).unwrap();
    let facts = facts();
    let prefs = Prefs { onboarded: true, followed_topics: vec!["science".into()], ..Default::default() };
    let weights = HashMap::new();
    let mut store = Store::memory()?;
    let mut today: Option<Edition> = None;
    for day in (0..3).rev() {
        let now = newest - Duration::days(day);
        let e = edition::build(edition::BuildInput {
            stories: stories(newest, day),
            facts: &facts,
            weights: &weights,
            prefs: &prefs,
            now,
            number: 3 - day as u32,
            sources_ok: OUTLETS.iter().map(|o| o.name.to_string()).collect(),
            sources_failed: vec![],
        });
        let rows: Vec<(String, String)> =
            e.pages.iter().flat_map(|p| &p.stories).map(|s| (s.story.id.clone(), serde_json::to_string(s).unwrap())).collect();
        store.save_edition(&e.date, &serde_json::to_string(&e)?, &rows)?;
        if day == 0 {
            today = Some(e);
        }
    }
    let today = today.expect("newest edition");

    // Open the lead story with a full text, as the app does once it fetches the article.
    let lead = &today.pages[0].stories[0];
    let cluster: Vec<OutletFacts> = std::iter::once(lead.report.outlet.clone())
        .chain(lead.others.iter().filter_map(|t| facts.values().find(|f| f.name == t.outlet_name).cloned()))
        .collect();
    let report = bias::assess(bias::Input {
        title: &lead.story.title,
        text: &BODY.join("\n\n"),
        url: &lead.story.url,
        full_text: true,
        outlet: lead.report.outlet.clone(),
        cluster,
    });
    store.set_story_body(&lead.story.id, &serde_json::to_string(BODY)?, &serde_json::to_string(&report)?)?;

    let shelf = json!([
        { "book": book("local:demo-almanac", BOOK_TITLE, BOOK_AUTHOR, &["Fiction", "Sea stories"], "A lighthouse keeper's year, one almanac line at a time."),
          "progress": 0.18, "location": null, "rating": null, "last_read": "2026-11-05T07:10:00Z", "format": "fb2" },
        { "book": book("demo:pride", "Pride and Prejudice", "Jane Austen", &["Fiction", "Courtship"], ""),
          "progress": 0.62, "location": null, "rating": 9, "last_read": "2026-11-03T21:00:00Z", "format": "epub" },
        { "book": book("demo:time-machine", "The Time Machine", "H. G. Wells", &["Science fiction"], ""),
          "progress": 1.0, "location": null, "rating": 8, "last_read": "2026-10-28T20:00:00Z", "format": "epub" },
        { "book": book("demo:middlemarch", "Middlemarch", "George Eliot", &["Fiction"], ""),
          "progress": 0.0, "location": null, "rating": null, "last_read": null, "format": "epub" },
        { "book": book("demo:herbal", "A Field Guide to Coastal Plants", "R. Penhallow", &["Botany"], ""),
          "progress": 0.33, "location": null, "rating": null, "last_read": "2026-10-30T12:00:00Z", "format": "pdf" },
    ]);

    let pubs = |list: &[(&str, &str, &str)]| -> Value {
        list.iter()
            .map(|(id, t, a)| {
                json!({ "id": format!("opds:{id}"), "title": t, "authors": [a], "summary": "", "cover": null,
                        "downloads": [{ "url": format!("https://library.example/get/{id}.epub"), "format": "epub" },
                                      { "url": format!("https://library.example/get/{id}.pdf"), "format": "pdf" }] })
            })
            .collect()
    };

    let out = json!({
        "edition": today,
        "editions": store.editions()?,
        "dates": store.edition_dates()?,
        "opened": { "id": lead.story.id, "paragraphs": BODY, "report": report, "note": null },
        "hits": store.search_stories("flood barrier", 20)?,
        "prefs": prefs,
        "topics": topics::TOPICS.iter().map(|t| json!({ "id": t.id, "label": t.label, "sensitive": t.sensitive })).collect::<Vec<_>>(),
        "library": shelf,
        "book": { "id": "local:demo-almanac", "fb2": fb2() },
        "comfort": { "settings": comfort::Settings::default_for("desktop", "day"), "question": null, "history": [] },
        "catalogues": [{ "id": "cdemo", "title": "Commons Library", "url": "https://library.example/opds", "user": "" }],
        "catalogue": {
            "title": "Commons Library", "url": "https://library.example/opds",
            "links": [
                { "title": "New this month", "url": "https://library.example/opds/new", "summary": "Recently added books", "cover": null },
                { "title": "Most read", "url": "https://library.example/opds/popular", "summary": "", "cover": null },
                { "title": "By subject", "url": "https://library.example/opds/subjects", "summary": "", "cover": null },
            ],
            "books": pubs(&[("persuasion", "Persuasion", "Jane Austen"), ("war-worlds", "The War of the Worlds", "H. G. Wells"),
                            ("silas", "Silas Marner", "George Eliot"), ("moonstone", "The Moonstone", "Wilkie Collins")]),
            "groups": [], "next": "https://library.example/opds?page=2", "prev": null,
            "search": "https://library.example/opds/search?q={searchTerms}",
        },
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
