//! `kmd search <query>` — search from CLI

use color_eyre::Result;

pub fn run(query: &str, limit: usize, json: bool) -> Result<()> {
    let config = super::load_config()?;

    // Check spelling query (@sp / @spell)
    if let Some(spell_query) =
        kmd_core::web::parse_spell_query_with_prefixes(query, &config.launcher.spell_prefixes)
    {
        let items = kmd_core::web::spell_result_items(
            &spell_query,
            &config.launcher.spell_providers,
            config.general.emoji_icons,
        );
        if json {
            print_items_json(&items)?;
        } else if spell_query.is_empty() {
            println!("Configured spelling providers:");
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
            let usage = config
                .launcher
                .spell_prefixes
                .first()
                .map(String::as_str)
                .unwrap_or("@sp");
            println!("\nUsage: {} <text>", usage);
        } else {
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
        }
        return Ok(());
    }

    // Check translate query (@tr / @trko / @tren)
    if let Some((direction, tr_query)) = kmd_core::web::parse_translate_query_with_prefixes(
        query,
        &config.launcher.translate_prefixes,
    ) {
        let items = kmd_core::web::translate_result_items(
            &tr_query,
            direction,
            &config.launcher.translate_providers,
            config.general.emoji_icons,
        );
        if json {
            print_items_json(&items)?;
        } else if tr_query.is_empty() {
            println!("Configured translate providers:");
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
            let usage = config
                .launcher
                .translate_prefixes
                .first()
                .map(String::as_str)
                .unwrap_or("@tr");
            println!("\nUsage: {} <text>", usage);
        } else {
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
        }
        return Ok(());
    }

    // Check for multi-LLM query (@llm / @multi / @cmp)
    if let Some((_services, llm_query)) = kmd_core::web::parse_multi_llm_query_with_prefixes(
        query,
        &config.launcher.multi_llm_providers,
        &config.launcher.multi_llm_prefixes,
    ) {
        let items = kmd_core::web::multi_llm_result_items(
            &llm_query,
            &config.launcher.multi_llm_providers,
            config.general.emoji_icons,
        );
        if json {
            print_items_json(&items)?;
        } else if llm_query.is_empty() {
            println!("Configured multi-LLM providers:");
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
            let usage = config
                .launcher
                .multi_llm_prefixes
                .first()
                .map(String::as_str)
                .unwrap_or("@llm");
            println!("\nUsage: {} <prompt>", usage);
        } else {
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
        }
        return Ok(());
    }

    // Check for multi-web query (@msearch / @multisearch / @searchall / @krsearch)
    if let Some((_services, web_query)) = kmd_core::web::parse_multi_web_query_with_prefixes(
        query,
        &config.launcher.multi_web_providers,
        &config.launcher.multi_web_prefixes,
    ) {
        let items = kmd_core::web::multi_web_result_items(
            &web_query,
            &config.launcher.multi_web_providers,
            config.general.emoji_icons,
        );
        if json {
            print_items_json(&items)?;
        } else if web_query.is_empty() {
            println!("Configured multi-web engines:");
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
            let usage = config
                .launcher
                .multi_web_prefixes
                .first()
                .map(String::as_str)
                .unwrap_or("@msearch");
            println!("\nUsage: {} <query>", usage);
        } else {
            for item in &items {
                println!("  {} {}", item.icon, item.name);
            }
        }
        return Ok(());
    }

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
            let item =
                kmd_core::web::search_result_item(service, &web_query, config.general.emoji_icons);
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
        println!(
            "Search: \"{}\" [{}] ({} results)\n",
            query,
            mode.label(),
            results.len()
        );
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
