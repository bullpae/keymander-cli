//! `kmd index` — manage the search index

use color_eyre::Result;

pub fn run(rebuild: bool, stats: bool) -> Result<()> {
    let config = super::load_config()?;
    let cache_path = super::index_cache_path();

    if stats && !rebuild {
        // Show stats for existing index
        if cache_path.exists() {
            let index = kmd_core::index::store::load_index(&cache_path)
                .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;
            print_stats(&index);
        } else {
            println!("No index found. Run `kmd index --rebuild` to create one.");
        }
        return Ok(());
    }

    if rebuild || !cache_path.exists() {
        // Show detected provider and priority dirs
        let provider = kmd_core::index::files::detect_provider(
            &config.launcher.file_search_provider,
            config.launcher.everything_path.as_ref(),
        );
        println!("File provider: {}", provider);

        if !config.launcher.search_paths.is_empty() {
            println!("Search paths:");
            for d in &config.launcher.search_paths {
                println!("  {}", d.display());
            }
        }
        if config.launcher.scan_drives {
            println!("Auto-scan drives: enabled (depth {})", config.launcher.drive_scan_depth);
        }

        println!("Building index...");
        let start = std::time::Instant::now();
        let index = kmd_core::Index::build(&config.launcher);
        let elapsed = start.elapsed();

        // Save to cache
        kmd_core::index::store::save_index(&index, &cache_path)
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        println!("Index built in {:.1}ms", elapsed.as_secs_f64() * 1000.0);
        print_stats(&index);
    } else {
        let index = kmd_core::index::store::load_index(&cache_path)
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        println!("Index loaded from cache.");
        print_stats(&index);
        println!("\nUse `kmd index --rebuild` to refresh.");
    }

    Ok(())
}

fn print_stats(index: &kmd_core::Index) {
    use kmd_core::index::Source;
    use std::collections::HashMap;

    let total = index.len();
    let apps = index.items.iter().filter(|i| i.source == Source::Apps).count();
    let path_exes = index.items.iter().filter(|i| i.source == Source::Path).count();
    let files = index.items.iter().filter(|i| i.source == Source::FileProvider).count();
    let sys_cmds = index.items.iter().filter(|i| i.source == Source::SystemCommand).count();

    println!("\nIndex Statistics:");
    println!("  Total items:      {}", total);
    println!("  Applications:     {}", apps);
    println!("  PATH executables: {}", path_exes);
    println!("  Files:            {}", files);
    println!("  System commands:  {}", sys_cmds);

    // Show top file extensions
    if files > 0 {
        let mut ext_counts: HashMap<String, usize> = HashMap::new();
        for item in index.items.iter().filter(|i| i.source == Source::FileProvider) {
            let ext = std::path::Path::new(&item.path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("(none)")
                .to_lowercase();
            *ext_counts.entry(ext).or_default() += 1;
        }
        let mut ext_vec: Vec<_> = ext_counts.into_iter().collect();
        ext_vec.sort_by(|a, b| b.1.cmp(&a.1));
        println!("\n  Top file extensions:");
        for (ext, count) in ext_vec.iter().take(15) {
            println!("    .{:<10} {}", ext, count);
        }
    }

    if let Some(ref ts) = index.last_updated {
        println!("\n  Last updated:     {}", ts);
    }
}
