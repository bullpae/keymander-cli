//! `kmd prompt` — 프롬프트 템플릿 관리 CLI

use color_eyre::eyre::bail;
use color_eyre::Result;

pub enum Action {
    List,
    Add { name: String, body: String },
    Remove { name: String },
}

pub fn run(action: Option<Action>) -> Result<()> {
    let mut config = super::load_config()?;

    match action {
        None | Some(Action::List) => {
            if config.launcher.prompt_templates.is_empty() {
                println!("저장된 프롬프트 템플릿이 없습니다.");
                println!("\n사용법:");
                println!("  kmd prompt add <name> \"<body>\"");
                println!("  예: kmd prompt add review \"다음 코드를 리뷰해주세요:\\n{{query}}\"");
                println!("\n@ll에서 사용:");
                println!("  @ll :review fn main() {{}}");
            } else {
                println!("프롬프트 템플릿 목록:\n");
                for t in &config.launcher.prompt_templates {
                    println!(
                        "  :{:<16} {}",
                        t.name,
                        kmd_core::prompt::preview_body(&t.body, 60)
                    );
                }
                println!("\n@ll에서 사용: @ll :<name> <query>");
            }
        }
        Some(Action::Add { name, body }) => {
            if !kmd_core::prompt::validate_template_name(&name) {
                bail!(
                    "잘못된 이름: '{}' (영문/숫자/하이픈/언더스코어만, 최대 32자)",
                    name
                );
            }
            if body.is_empty() {
                bail!("본문이 비어 있습니다");
            }

            // 기존 동일 이름 제거 후 추가
            config
                .launcher
                .prompt_templates
                .retain(|t| !t.name.eq_ignore_ascii_case(&name));
            config
                .launcher
                .prompt_templates
                .push(kmd_core::config::PromptTemplate {
                    name: name.clone(),
                    body: body.clone(),
                });
            config.save()?;
            println!("✅ 템플릿 '{}' 저장됨", name);
            println!("   @ll :{} <query> 형태로 사용할 수 있습니다", name);
        }
        Some(Action::Remove { name }) => {
            let before = config.launcher.prompt_templates.len();
            config
                .launcher
                .prompt_templates
                .retain(|t| !t.name.eq_ignore_ascii_case(&name));
            if config.launcher.prompt_templates.len() < before {
                config.save()?;
                println!("✅ 템플릿 '{}' 삭제됨", name);
            } else {
                bail!("템플릿 '{}'을 찾을 수 없습니다", name);
            }
        }
    }

    Ok(())
}
