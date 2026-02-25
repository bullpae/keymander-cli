//! OS-specific application discovery
//!
//! Discovers installed applications:
//! - Windows: Start Menu shortcuts (.lnk files)
//! - macOS: /Applications/*.app
//! - Linux: .desktop files in XDG directories

use super::{IndexItem, ItemKind, Source};

/// Icon for applications
fn app_icon(use_emoji: bool) -> String {
    if use_emoji {
        "\u{1F4E6}".into()
    } else {
        "Ap".into()
    } // 📦 / Ap
}

/// Collect installed applications from OS-specific locations
pub fn collect_apps(use_emoji: bool) -> Vec<IndexItem> {
    let mut items = Vec::new();

    #[cfg(target_os = "windows")]
    {
        items.extend(collect_windows_apps(use_emoji));
    }

    #[cfg(target_os = "macos")]
    {
        items.extend(collect_macos_apps(use_emoji));
    }

    #[cfg(target_os = "linux")]
    {
        items.extend(collect_linux_apps(use_emoji));
    }

    items
}

/// Windows: Discover installed applications from multiple sources.
///
/// 1. **`shell:AppsFolder`** — the authoritative Windows app list (includes
///    UWP/Store/MSIX apps like Telegram, Zed, Bitwarden, etc. as well as
///    traditional Win32 apps).  Enumerated via a short PowerShell script.
/// 2. **Start Menu `.lnk` files** — fallback that catches anything
///    `shell:AppsFolder` might miss (rare, but possible for legacy installers).
///
/// Results are deduplicated by name (case-insensitive).
#[cfg(target_os = "windows")]
fn collect_windows_apps(use_emoji: bool) -> Vec<IndexItem> {
    use std::collections::HashSet;
    use std::path::PathBuf;

    let mut items = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // ── Source 1: shell:AppsFolder (PowerShell COM) ─────────────────────
    // This is the same data source Windows Start menu search uses.
    items.extend(collect_shell_apps_folder(&mut seen, use_emoji));

    // ── Source 2: Start Menu .lnk files (fallback) ─────────────────────
    let mut dirs: Vec<PathBuf> = Vec::new();

    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(
            PathBuf::from(&appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    if let Ok(programdata) = std::env::var("PROGRAMDATA") {
        dirs.push(
            PathBuf::from(&programdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    for dir in dirs {
        scan_lnk_dir(&dir, &mut items, &mut seen, use_emoji);
    }

    items
}

/// Enumerate `shell:AppsFolder` via PowerShell COM.
///
/// Runs a small PowerShell one-liner that outputs tab-separated lines:
///   `Name\tPath`
///
/// Apps whose path looks like a regular filesystem executable get that path.
/// UWP/Store apps get an `shell:appsFolder\<id>` URI that `explorer.exe` can launch.
///
/// Timeout: 5 seconds. On failure, returns an empty vec (the .lnk fallback
/// will still run).
#[cfg(target_os = "windows")]
fn collect_shell_apps_folder(
    seen: &mut std::collections::HashSet<String>,
    use_emoji: bool,
) -> Vec<IndexItem> {
    use std::process::Command;

    // PowerShell script: enumerate shell:AppsFolder, output Name<TAB>Path
    // Filters out framework/runtime packages by skipping names that contain
    // common noise patterns.
    let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$shell = New-Object -ComObject Shell.Application
$folder = $shell.NameSpace('shell:AppsFolder')
foreach ($item in $folder.Items()) {
    $n = $item.Name
    $p = $item.Path
    if ($n -and $p) { "$n`t$p" }
}
"#;

    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        ps_script,
    ]);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            tracing::debug!(
                "shell:AppsFolder PowerShell failed (exit {:?})",
                o.status.code()
            );
            return Vec::new();
        }
        Err(e) => {
            tracing::debug!("shell:AppsFolder PowerShell error: {}", e);
            return Vec::new();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut items = Vec::new();

    // Noise patterns to skip (framework/runtime/extension packages)
    let skip_patterns = [
        "Microsoft.WinAppRuntime",
        "Microsoft.VCLibs",
        "Microsoft.UI.Xaml",
        "Microsoft.NET.",
        "Microsoft.D3D",
        "Microsoft.DirectX",
        "Microsoft.Services",
        "MicrosoftWindows.UndockedDevKit",
        "Microsoft.ApplicationCompatibility",
        "Microsoft.Ink.Handwriting",
        "Microsoft.LanguageExperience",
        "DesktopAppInstaller",
        "StorePurchaseApp",
        "WidgetsPlatform",
        "WinAppRuntime",
        "CrossDevice",
        "ShellExperienceHost",
        "StartExperiencesApp",
        "Debuggable Package",
    ];

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Some((name, path)) = line.split_once('\t') else {
            continue;
        };

        let name = name.trim().to_string();
        let path = path.trim().to_string();

        if name.is_empty() || path.is_empty() {
            continue;
        }

        // Skip noise
        let name_lower = name.to_lowercase();
        if name_lower.contains("uninstall") || name_lower.contains("제거") {
            continue;
        }
        if skip_patterns.iter().any(|p| path.contains(p)) {
            continue;
        }

        // Dedup by lowercase name
        if !seen.insert(name_lower) {
            continue;
        }

        // Determine the launchable path:
        // - Filesystem paths start with a drive letter (e.g. "C:\...")
        // - Everything else (CLSID, AUMID, PackageFamilyName) needs shell:appsFolder\ URI
        let is_filesystem_path = path.len() >= 3
            && path.as_bytes()[0].is_ascii_alphabetic()
            && path.as_bytes()[1] == b':'
            && path.as_bytes()[2] == b'\\';
        let launch_path = if is_filesystem_path {
            path.clone()
        } else {
            format!("shell:appsFolder\\{}", path)
        };

        items.push(IndexItem {
            name,
            path: launch_path.clone(),
            kind: ItemKind::App,
            source: Source::Apps,
            icon: app_icon(use_emoji),
            keywords: format!("{} {}", launch_path, path),
        });
    }

    tracing::info!("shell:AppsFolder discovered {} apps", items.len());
    items
}

#[cfg(target_os = "windows")]
fn scan_lnk_dir(
    dir: &std::path::Path,
    items: &mut Vec<IndexItem>,
    seen: &mut std::collections::HashSet<String>,
    use_emoji: bool,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_lnk_dir(&path, items, seen, use_emoji);
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        if ext.to_lowercase() != "lnk" {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }

        // Skip common uninstallers
        let name_lower = name.to_lowercase();
        if name_lower.contains("uninstall") || name_lower.contains("제거") {
            continue;
        }

        let full_path = path.to_string_lossy().to_string();

        items.push(IndexItem {
            name,
            path: full_path.clone(),
            kind: ItemKind::App,
            source: Source::Apps,
            icon: app_icon(use_emoji),
            keywords: full_path,
        });
    }
}

/// macOS: Scan /Applications for .app bundles
#[cfg(target_os = "macos")]
fn collect_macos_apps(use_emoji: bool) -> Vec<IndexItem> {
    use std::path::PathBuf;

    let mut items = Vec::new();

    let app_dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        dirs::home_dir()
            .map(|h| h.join("Applications"))
            .unwrap_or_default(),
    ];

    for dir in app_dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };

            if ext != "app" {
                continue;
            }

            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                continue;
            }

            let full_path = path.to_string_lossy().to_string();

            items.push(IndexItem {
                name,
                path: full_path.clone(),
                kind: ItemKind::App,
                source: Source::Apps,
                icon: app_icon(use_emoji),
                keywords: full_path,
            });
        }
    }

    items
}

/// Linux: Parse .desktop files from XDG data directories
#[cfg(target_os = "linux")]
fn collect_linux_apps(use_emoji: bool) -> Vec<IndexItem> {
    use std::collections::HashSet;
    use std::path::PathBuf;

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    let mut dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];

    if let Some(data_home) = dirs::data_dir() {
        dirs.push(data_home.join("applications"));
    }

    // XDG_DATA_DIRS
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for dir in xdg.split(':') {
            dirs.push(PathBuf::from(dir).join("applications"));
        }
    }

    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if ext != "desktop" {
                continue;
            }

            if let Some(item) = parse_desktop_file(&path, &mut seen, use_emoji) {
                items.push(item);
            }
        }
    }

    items
}

/// Parse a .desktop file and extract name/exec
#[cfg(target_os = "linux")]
fn parse_desktop_file(
    path: &std::path::Path,
    seen: &mut std::collections::HashSet<String>,
    use_emoji: bool,
) -> Option<IndexItem> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut name = None;
    let mut exec = None;
    let mut no_display = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }
        if !in_desktop_entry {
            continue;
        }

        if let Some(val) = line.strip_prefix("Name=") {
            name = Some(val.to_string());
        } else if let Some(val) = line.strip_prefix("Exec=") {
            // Remove field codes like %u, %f, %U, etc.
            let clean = val
                .split_whitespace()
                .filter(|s| !s.starts_with('%'))
                .collect::<Vec<_>>()
                .join(" ");
            exec = Some(clean);
        } else if let Some(val) = line.strip_prefix("NoDisplay=") {
            no_display = val.trim().eq_ignore_ascii_case("true");
        }
    }

    if no_display {
        return None;
    }

    let name = name?;
    let exec = exec.unwrap_or_default();

    if !seen.insert(name.clone()) {
        return None;
    }

    Some(IndexItem {
        name: name.clone(),
        path: exec.clone(),
        kind: ItemKind::App,
        source: Source::Apps,
        icon: app_icon(use_emoji),
        keywords: format!("{} {}", name, exec),
    })
}
