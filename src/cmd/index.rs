//! `kmd index` — manage the search index

use color_eyre::Result;

pub fn run(rebuild: bool, stats: bool) -> Result<()> {
    let config = super::load_config()?;
    let cache_path = kmd_core::Config::default_data_dir().join("index.json");

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

    if let Some(ref ts) = index.last_updated {
        println!("  Last updated:     {}", ts);
    }
}
