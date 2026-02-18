//! `kmd keymap` — keymap backend control (Kanata)

use color_eyre::Result;

pub enum Action {
    Start,
    Stop,
    Status,
    List,
    Use { profile: String },
    Init { profile: Option<String> },
    ListPresets,
}

pub fn run(action: Action) -> Result<()> {
    let mut config = super::load_config()?;

    match action {
        Action::Start => match kmd_core::keymap::start(&config) {
            Ok(msg) => println!("{msg}"),
            Err(e) => println!("{e}"),
        },
        Action::Stop => match kmd_core::keymap::stop() {
            Ok(msg) => println!("{msg}"),
            Err(e) => println!("{e}"),
        },
        Action::Status => {
            println!("keymap status: {}", kmd_core::keymap::status());
            println!(
                "backend: {}, active_profile: {}",
                config.launcher.keymap.backend, config.launcher.keymap.active_profile
            );
        }
        Action::List => {
            let profiles = kmd_core::keymap::list_profiles(&config);
            if profiles.is_empty() {
                println!("등록된 keymap 프로파일이 없습니다.");
                println!("힌트: `kmd keymap init` 으로 기본 프리셋을 설치하세요.");
            } else {
                println!("keymap profiles:");
                for p in profiles {
                    if p == config.launcher.keymap.active_profile {
                        println!("  * {p} (active)");
                    } else {
                        println!("  - {p}");
                    }
                }
            }
        }
        Action::Use { profile } => {
            kmd_core::keymap::validate_profile_name(&profile)
                .map_err(|e| color_eyre::eyre::eyre!(e))?;
            let profile_name = kmd_core::keymap::with_extension(&profile);
            let path = kmd_core::keymap::profile_dir(&config).join(&profile_name);
            if !kmd_core::keymap::exists(&path) {
                return Err(color_eyre::eyre::eyre!(format!(
                    "프로파일 파일이 없습니다: {}\n먼저 `kmd keymap init {}` 실행을 권장합니다.",
                    path.display(),
                    profile_name
                )));
            }
            config.launcher.keymap.active_profile = profile_name.clone();
            config.save()?;
            println!("active profile 변경: {}", profile_name);
        }
        Action::Init { profile } => {
            let profile_name = profile.unwrap_or_else(|| "vim-nav".to_string());
            kmd_core::keymap::validate_profile_name(&profile_name)
                .map_err(|e| color_eyre::eyre::eyre!(e))?;
            let created = kmd_core::keymap::create_profile_template(&config, &profile_name)
                .map_err(|e| color_eyre::eyre::eyre!(e))?;
            let final_name = created
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| profile_name.clone());
            config.launcher.keymap.active_profile = final_name.clone();
            config.save()?;
            println!("프로파일 준비 완료: {}", created.display());
            println!("active profile: {final_name}");

            if kmd_core::keymap::preset_content(&profile_name).is_some() {
                println!("(내장 프리셋 '{profile_name}' 사용)");
            }
        }
        Action::ListPresets => {
            println!("사용 가능한 내장 프리셋:");
            for (name, desc) in kmd_core::keymap::list_presets() {
                println!("  {name:<12} {desc}");
            }
            println!();
            println!("설치: kmd keymap init <preset>");
        }
    }

    Ok(())
}
