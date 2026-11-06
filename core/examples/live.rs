//! Live smoke test: build today's paper from the real feeds and print it.
//! `cargo run -p granthalaya-core --example live`
use std::collections::HashMap;

use granthalaya_core::{edition, feeds, sources, wikidata};

#[tokio::main]
async fn main() {
    let client = feeds::client();
    let t = std::time::Instant::now();
    let (stories, ok, failed) = edition::gather(&client).await;
    println!("{} stories from {} outlets in {:?}; failed: {:?}", stories.len(), ok.len(), t.elapsed(), failed);
    let t = std::time::Instant::now();
    let all: Vec<&sources::Outlet> = sources::outlets().iter().collect();
    let mut facts = HashMap::new();
    match wikidata::fetch_many(&client, &all).await {
        Ok(list) => {
            println!("wikidata facts for {} outlets in {:?}", list.len(), t.elapsed());
            for f in list {
                println!(
                    "{}: owners {:?} funders {:?}",
                    f.name,
                    f.owners.iter().map(|l| &l.label).collect::<Vec<_>>(),
                    f.funders.iter().map(|l| &l.label).collect::<Vec<_>>()
                );
                facts.insert(f.id.clone(), f);
            }
        }
        Err(e) => println!("wikidata failed {e}"),
    }
    let t = std::time::Instant::now();
    let e = edition::build(edition::BuildInput {
        stories,
        facts: &facts,
        weights: &HashMap::new(),
        prefs: &Default::default(),
        now: chrono::Utc::now(),
        number: 1,
        sources_ok: ok,
        sources_failed: failed,
    });
    println!("built in {:?}", t.elapsed());
    for p in &e.pages {
        println!("\n== {} ({})", p.title, p.stories.len());
        for s in &p.stories {
            println!(
                "  [{:>3} {}] {} — {} (+{} tellings; {})",
                s.report.score,
                s.report.verdict,
                s.story.title,
                s.story.outlet_name,
                s.others.len(),
                s.story.topics.join(",")
            );
        }
    }
    let lead = &e.pages[0].stories[0];
    println!("\nLEAD REPORT: {}", serde_json::to_string_pretty(&lead.report).unwrap());
    let t = std::time::Instant::now();
    match granthalaya_core::article::fetch(&client, &lead.story.url).await {
        Ok(p) => println!("full text: {} paragraphs in {:?}; first: {}", p.len(), t.elapsed(), p[0]),
        Err(e) => println!("full text failed: {e}"),
    }
}
