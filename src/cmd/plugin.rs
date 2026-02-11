//! `kmd plugin` — manage plugins

use color_eyre::Result;

pub enum Action {
    List,
}

pub fn run(action: Action) -> Result<()> {
    match action {
        Action::List => {
            let plugin_dir = kmd_core::plugin::loader::default_plugin_dir();
            let plugins = kmd_core::plugin::loader::discover_plugins(&plugin_dir);

            if plugins.is_empty() {
                println!("No plugins installed.");
                println!("\nPlugin directory: {}", plugin_dir.display());
            } else {
                println!("Installed plugins:\n");
                for (path, manifest) in &plugins {
                    println!(
                        "  {} v{} — {}",
                        manifest.name, manifest.version, manifest.description
                    );
                    if let Some(ref prefix) = manifest.prefix {
                        println!("    Prefix: {}", prefix);
                    }
                    println!("    Path: {}", path.display());
                    println!();
                }
            }
        }
    }

    Ok(())
}
