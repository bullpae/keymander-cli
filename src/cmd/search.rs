//! `kmd search <query>` — search from CLI

use color_eyre::Result;

pub fn run(query: &str, limit: usize, json: bool) -> Result<()> {
    let config = super::load_config()?;

    // Check for web query (@prefix)
    if let Some((service, web_query)) = kmd_core::web::parse_web_query(query) {
        if web_query.is_empty() {
            // List services
            let items = kmd_core::web::list_services_as_items("", config.general.emoji_icons);
            if json {
                print_items_json(&items)?;
            } else {
                for item in &items {
                    println!("  {} {}", item.icon, item.name);
                }
            }
        } else {
            let item = kmd_core::web::search_result_item(service, &web_query, config.general.emoji_icons);
            let url = kmd_core::web::build_search_url(service, &web_query);
            if json {
                print_items_json(&[item])?;
            } else {
                println!("  {} {} → {}", service.icon, service.name, url);
            }
        }
        return Ok(());
    }

    let mut engine = super::create_search_engine(&config);

    let (mode, mut results) = engine.search(query, limit);

    // Apply history boost
    if let Ok(db) = super::open_db() {
        kmd_core::history::boost_results(&mut results, &db);
    }

    if json {
        let items: Vec<_> = results.iter().map(|r| &r.item).collect();
        let json_str = serde_json::to_string_pretty(&items)?;
        println!("{}", json_str);
    } else {
        println!("Search: \"{}\" [{}] ({} results)\n", query, mode.label(), results.len());
        for (i, result) in results.iter().enumerate() {
            println!(
                "  {:>2}. {} {:<30} [{}]  {}",
                i + 1,
                result.item.icon,
                result.item.name,
                result.item.kind,
                result.item.path,
            );
        }
    }

    Ok(())
}

fn print_items_json(items: &[kmd_core::index::IndexItem]) -> color_eyre::Result<()> {
    let json = serde_json::to_string_pretty(items)?;
    println!("{}", json);
    Ok(())
}
