# keymander 문서

문서마다 **성격이 다르다.** 어떤 것은 현재 동작의 정본이고, 어떤 것은 이미
구현된 기능의 설계 이력이며, 어떤 것은 아직 구현되지 않은 계획이다. 이걸 구분하지
않으면 "계획서를 읽고 구현된 줄 알았다" 또는 그 반대의 오해가 생긴다.

## 참조 — 현재 동작의 정본

바꾸려면 코드와 함께 고쳐야 하는 문서들.

| 문서 | 내용 |
|---|---|
| [01_prd.md](01_prd.md) | 제품 요구사항 — 무엇을 만드는가 |
| [02_architecture.md](02_architecture.md) | 시스템 구조, 크레이트 경계, 데이터 흐름 |
| [03_plugin_spec.md](03_plugin_spec.md) | 플러그인 인터페이스 |
| [04_database_schema.md](04_database_schema.md) | DB 스키마 |
| [05_theming.md](05_theming.md) | 테마 규격 |
| [06_config_reference.md](06_config_reference.md) | `config.toml` 전체 레퍼런스 |
| [07_distribution.md](07_distribution.md) | 배포 채널(brew/apt/yum/winget) 운영 가이드 + 공개 저장소 위생 |
| [versioning-and-release.md](versioning-and-release.md) | 버전·릴리스 규칙 |

## 설계 이력 — 구현 완료된 기능의 "왜"

기능은 이미 릴리스됐다. 이 문서들이 남아 있는 이유는 **검토한 대안과 기각한
이유**가 코드에는 안 남기 때문이다. 해당 코드를 고치기 전에 읽으면 같은 함정을
다시 밟지 않는다. 단, 서술이 미래형이라도 계획이 아니라 기록이다.

| 문서 | 상태 | 구현 위치 |
|---|---|---|
| [08_layer_passthrough_plan.md](08_layer_passthrough_plan.md) | ✅ v0.9.3 (P0–P3) | `crates/kmd-daemon/src/keybind/engine.rs` |
| [09_llm_autopilot_plan.md](09_llm_autopilot_plan.md) | ✅ v0.10.0 (Windows 전용) | `crates/kmd-daemon/src/autopilot.rs` |

## 계획 — 아직 구현되지 않음

| 문서 | 상태 |
|---|---|
| [10_dojo_plan.md](10_dojo_plan.md) | 미구현 (2026-08-08 확인) |
| [deferred-ideas.md](deferred-ideas.md) | v0.3 로드맵에서 보류한 아이디어 모음 — 재검토용 |

## 변경 이력

- [CHANGELOG.md](../CHANGELOG.md) — 0.4.0 이후 전 버전
- [v03-changelog.md](v03-changelog.md) — 0.3.x 상세 (CHANGELOG의 `[0.3.x] and earlier`가 가리킨다)

---

**문서를 추가하거나 기능을 릴리스할 때**: 계획 문서가 구현되면 이 표에서 "계획"
→ "설계 이력"으로 옮기고 문서 상단에 상태 배너를 단다. 그대로 두면 다음 사람이
이미 있는 걸 다시 만들려 든다.
