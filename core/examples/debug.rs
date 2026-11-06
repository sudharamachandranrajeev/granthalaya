use granthalaya_core::{cluster, edition, feeds, sources};
#[tokio::main]
async fn main() {
    let c = feeds::client();
    for id in ["cbc"] {
        let o = sources::outlet(id).unwrap();
        match feeds::fetch(&c, o, &o.feeds[0]).await { Ok(s) => println!("{id} ok {}", s.len()), Err(e) => println!("{id} ERR {e:#}") }
    }
    if std::env::args().count() < 2 { return }
    let (stories, _, _) = edition::gather(&c).await;
    let cl = cluster::cluster(&stories);
    let mut big: Vec<_> = cl.iter().filter(|m| m.len() >= 4).collect();
    big.sort_by_key(|m| std::cmp::Reverse(m.len()));
    for m in big.iter().take(4) { println!("--- {}", m.len()); for &i in m.iter() { println!("   {} | {}", stories[i].outlet_name, stories[i].title); } }
}
