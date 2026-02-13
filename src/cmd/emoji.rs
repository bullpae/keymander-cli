//! `kmd emoji <query>` — search and copy emoji from CLI

use color_eyre::Result;
use kmd_core::plugin::builtin_emoji::EmojiExtension;

pub fn run(query: &str, list: bool, json: bool, copy_nth: Option<usize>) -> Result<()> {
    let ext = EmojiExtension;
    let results = ext.search_emoji(query);

    if results.is_empty() {
        if json {
            println!("[]");
        } else {
            eprintln!("No emoji found for \"{}\"", query);
        }
        return Ok(());
    }

    if json {
        // JSON output — extract name by stripping the emoji prefix from item.name
        let items: Vec<serde_json::Value> = results
            .iter()
            .enumerate()
            .map(|(i, item)| {
                // item.name = "😀 grinning face", item.path = "😀"
                let name = item
                    .name
                    .strip_prefix(&item.path)
                    .unwrap_or(&item.name)
                    .trim();
                // keywords = "grinning face Smileys & Emotion: face-smiling"
                // category is encoded after the name portion
                let category = item
                    .keywords
                    .strip_prefix(name)
                    .unwrap_or("")
                    .trim();
                serde_json::json!({
                    "index": i + 1,
                    "emoji": item.path,
                    "name": name,
                    "category": category,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    // Determine which emoji to copy
    let copy_index = if list {
        None // --list: don't copy anything
    } else {
        Some(copy_nth.unwrap_or(1)) // default: copy first result
    };

    // Print results
    for (i, item) in results.iter().enumerate() {
        let marker = if copy_index == Some(i + 1) { " ←" } else { "" };
        println!(
            "  {:>2}. {} {}{}",
            i + 1,
            item.path,  // the emoji character
            item.name.strip_prefix(&format!("{} ", item.path)).unwrap_or(&item.name),
            marker,
        );
    }

    // Copy to clipboard
    if let Some(idx) = copy_index {
        if idx >= 1 && idx <= results.len() {
            let emoji = &results[idx - 1].path;
            match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(emoji.clone())) {
                Ok(()) => {
                    println!("\n  ✅ Copied: {}", emoji);
                }
                Err(e) => {
                    eprintln!("\n  ❌ Failed to copy: {}", e);
                }
            }
        } else {
            eprintln!("\n  ❌ Index {} out of range (1-{})", idx, results.len());
        }
    }

    Ok(())
}
