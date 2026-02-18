# Keymander v0.3.x 변경 내역

---

## v0.3.3 (2026-02-18)

### 안정화/성능 개선

- **입력창 미표시 방어**: 저장된 창 좌표가 화면 영역 밖으로 판단되면 시작 시 자동 재센터링
- **창 상태 복원 안전화**: `window_state` 로드 시 비정상 값(NaN/inf) 제거, 너비를 `420~1200` 범위로 클램프
- **핫키 연타 안정화**: 싱글 인스턴스 토글에 debounce(`700ms`)를 추가해 즉시 꺼지는 현상 완화
- **첫 실행 지연 완화**: 인덱스 캐시 버전을 앱 버전과 분리(`schema-2`)하여 패치/마이너 업데이트 때 불필요한 전체 재인덱싱 방지

---

## v0.3.2 (2026-02-17)

### Kanata 키맵 통합

- **내장 프리셋**: `vim-nav`, `minimal` — `kmd keymap init vim-nav` / `kmd keymap list-presets`
- **vim-nav**: Alt 홀드 → Vim 스타일 HJKL 네비게이션 + Alt+Space → kmd-desktop 실행 (`cmd kmd-desktop`)
- **Desktop `:keymap` / `:km`**: 상태 표시, on/off, 프로파일 전환
- **CLI**: `kmd keymap start/stop/status/list/use/init/list-presets`

### 프로그램 아이콘

- **새 keymander 아이콘**: 픽셀아트 스타일(k>>r) 적용
- **kmd.exe, kmd-desktop.exe**: ICO 임베드 (탐색기/작업표시줄)
- **kmd-desktop 윈도우**: PNG 기반 32x32 아이콘
- **gen-icon**: PNG→ICO 변환기로 전환 (`cargo run --manifest-path tools/gen-icon/Cargo.toml`)

---

## v0.3.1 (2026-02-17)

### kmd-desktop: 브랜드 아이콘 적용

- **이모지 → PNG 로고**: Google, YouTube, GitHub, ChatGPT, Naver, Daum 등 20개 서비스의 실제 브랜드 아이콘 표시
- **아이콘 깜빡임 수정**: `Handle::from_bytes()` 매 프레임 호출로 인한 재디코딩 제거 → `LazyLock<HashMap>` 캐시로 1회 생성 후 clone 재사용
- **영향**: `crates/kmd-desktop/src/brand_icons.rs` 신규, `app.rs` `view_result_row()` image 위젯 적용

---

## v0.3.0

> Phase 0 (구조 리팩토링) + Phase 1 (프롬프트 템플릿) + Phase 2 (Quick Transform) + Phase 3 (Smart Directory Jump) 작업 요약

### Phase 0: 구조 리팩토링

### 0-1. web.rs → web/ 모듈 디렉토리 분리

**변경 전**: `kmd-core/src/web.rs` 단일 파일 (1121줄)

**변경 후**: `kmd-core/src/web/` 디렉토리 구조

| 파일 | 역할 | 주요 내용 |
|------|------|-----------|
| `mod.rs` | 모듈 루트 + re-export | 기존 `web::*` 임포트 호환 유지, 테스트 포함 |
| `services.rs` | 서비스 정의 + 상수 | `WebService`, `SpellService`, `TranslateService` 구조체, `WEB_SERVICES` 등 상수, `HasId` trait, `select_services` 공용 헬퍼 |
| `parsers.rs` | 쿼리 파싱 | `parse_web_query`, `parse_multi_*`, `classify_web_query` 통합 분류기, `WebQueryResult` enum, `WebQueryConfig` 설정 묶음 |
| `items.rs` | 결과 아이템 생성 | `list_services_as_items`, `multi_llm_result_items`, `extract_batch_urls` 통합 URL 추출, `build_batch_items` 공용 헬퍼 |

**중복 제거**:
- 4개의 `selected_*_services` 함수 → 제네릭 `select_services<T: HasId>` 1개로 통합
- 4개의 `extract_*_urls` 함수 → `extract_urls_with_prefix` 공용 로직 + `extract_batch_urls` 통합 함수
- `list_services_as_items`의 가상 항목 → 데이터 테이블로 정리

### 0-2. extract_batch_urls 통합 함수

`web::extract_batch_urls(item)` — translate, spell, multi_web, multi_llm URL을 한 번의 호출로 추출.

TUI/Desktop의 4-way if-let 체인을 단일 호출로 교체.

### 0-3. classify_web_query 통합 파서

`web::classify_web_query(input, &cfg) → WebQueryResult` — @ 쿼리를 한 번에 분류.

**WebQueryResult enum**:
- `Spell(query)`
- `Translate(direction, query)`
- `MultiLlm(services, query)`
- `MultiWeb(services, query)`
- `Single(service, query)`
- `Browse(filter)`

TUI `handle_web_query`와 Desktop `handle_web_query`의 if-else 체인을 `match` 문으로 교체.

---

## Phase 1: 프롬프트 템플릿 (:prompt)

### 새 모듈: `kmd-core/src/prompt.rs`

- `PromptTemplate` 구조체 (`config.rs`에 정의, `launcher.prompt_templates` 필드)
- `apply_template(templates, query)` — `:name rest` 형태 감지 → 템플릿 본문과 결합
  - `{query}` 자리표시자: 치환
  - 자리표시자 없음: `body + "\n\n" + rest`
- `validate_template_name` — 영문/숫자/하이픈/언더스코어, 최대 32자
- `list_templates_as_items` — TUI/Desktop에서 `:prompt` 검색 시 목록 표시

### CLI: `kmd prompt` 서브커맨드

```
kmd prompt              # 목록 표시
kmd prompt list         # 목록 표시
kmd prompt add <name> "<body>"    # 추가 (기존 동일 이름은 덮어쓰기)
kmd prompt remove <name>          # 삭제
```

### TUI/Desktop: `:prompt` prefix

- `:prompt` / `:pt` → 템플릿 목록 표시
- `:prompt add <name> <body>` → 저장
- `:prompt remove <name>` → 삭제

### @ll 템플릿 자동 결합

`@ll :review fn main() {}` 입력 시:
1. `parse_multi_llm_query_with_prefixes`로 쿼리 추출: `:review fn main() {}`
2. `apply_template`로 변환: `다음 코드를 리뷰해주세요:\nfn main() {}`
3. 변환된 최종 프롬프트를 클립보드에 복사 + LLM 탭 열기

---

## Phase 2: Quick Transform (:t)

### 새 모듈: `kmd-core/src/transform.rs`

- `TransformKind` enum: `Spell`, `Translate(direction)`
- `TransformQuery` 구조체: `kind` + `text`
- `parse_transform_query` — `:t spell/tr/trko/tren [text]` 파싱
- `help_items` — `:t` 만 입력 시 도움말 생성
- `build_transform_urls` — 설정된 provider에 대해 URL 목록 생성

### 사용법

| 명령 | 동작 |
|------|------|
| `:t spell <text>` | 맞춤법 검사 서비스 열기 |
| `:t spell` | 클립보드 내용으로 맞춤법 검사 |
| `:t tr <text>` | 자동 감지 번역 |
| `:t trko <text>` | 영어 → 한국어 번역 |
| `:t tren <text>` | 한국어 → 영어 번역 |
| `:t` | 도움말 표시 |

텍스트 생략 시 클립보드 내용을 자동으로 사용.

---

## Phase 3: Smart Directory Jump

### 개요

zoxide 스타일의 멀티 토큰 경로 매칭 + frecency 기반 학습 기능.
기존 `filter_contains()`를 업그레이드하여 `SearchMode` 추가 없이 구현.

### 멀티 토큰 AND 매칭 (`search.rs`)

- `split_whitespace()` + 경로 구분자 분리로 토큰 추출
- 토큰 2개 이상: 모든 토큰이 name/path/keywords 중 하나에 포함되어야 매칭 (AND)
- 가중 스코어링:
  - 경로 세그먼트 정확 일치: +60
  - 경로 내 substring: +30
  - 이름 매칭: +15
  - 키워드 매칭: +5
  - 전 토큰 세그먼트 정확 일치 보너스: +80
- 단일 토큰은 기존 동작 100% 보존 (score: 0)

| 예시 | 동작 |
|------|------|
| `2026 출장이력` | `c:\2026\work\출장이력` 매칭 (세그먼트 정확 일치 보너스) |
| `출장이력 2026` | 토큰 순서 무관, 동일 결과 |
| `project 보고서` | 한영 혼합, non-ASCII이므로 Contains → 멀티 토큰 AND |
| `출장이력` | 단일 토큰, 기존 Contains 동작 그대로 |

### Frecency 알고리즘 (`history.rs`)

- `frecency_score(frequency, executed_at)` = frequency × recency_weight
- 시간 감쇠 가중치: 1시간 이내(×16), 24시간(×8), 1주(×4), 1달(×2), 이후(×1)
- `FRECENCY_BOOST_CAP = 200`: Score Pollution 방지
- `parse_hours_ago()`: 외부 크레이트 없이 ISO 8601 수동 파싱

### History Pruning (`db.rs`)

- `prune_history()`: zoxide 스타일 aging 알고리즘
  - 전체 frequency 합 > 9000 시, 각 행 frequency를 0.9배 감쇠
  - frequency < 1 인 행 자동 삭제
- `record_launch()` 내 확률적 호출 (~5%)
- DB migration v2: `executed_at` 인덱스 추가

### 잠재 문제 대응

| 문제 | 대응 |
|------|------|
| Score Pollution | search_score(~200) >= frecency(~200, capped) 균형 |
| History 무한 증식 | aging + pruning 메커니즘 |
| Windows 경로 엣지 케이스 | `\` / `/` 동시 분리, 빈 세그먼트 필터링 |
| 네트워크 경로 블로킹 | hot path에 `exists()` 없음 |
| 시간 파싱 실패 | `parse_hours_ago()` → `None` → weight=1 fallback |

---

## 파일 변경 요약

### 새로 생성된 파일

| 파일 | 설명 |
|------|------|
| `crates/kmd-core/src/web/mod.rs` | web 모듈 루트 |
| `crates/kmd-core/src/web/services.rs` | 서비스 정의/상수 |
| `crates/kmd-core/src/web/parsers.rs` | 쿼리 파서 + 분류기 |
| `crates/kmd-core/src/web/items.rs` | 결과 아이템 빌더 |
| `crates/kmd-core/src/prompt.rs` | 프롬프트 템플릿 |
| `crates/kmd-core/src/transform.rs` | Quick Transform |
| `src/cmd/prompt.rs` | CLI prompt 서브커맨드 |
| `docs/deferred-ideas.md` | 보류 아이디어 문서 |
| `docs/v03-changelog.md` | 이 문서 |

### 삭제된 파일

| 파일 | 사유 |
|------|------|
| `crates/kmd-core/src/web.rs` | web/ 디렉토리로 분리 |

### 수정된 파일

| 파일 | 변경 내용 |
|------|-----------|
| `crates/kmd-core/src/lib.rs` | `prompt`, `transform` 모듈 추가 |
| `crates/kmd-core/src/config.rs` | `PromptTemplate` 구조체, `prompt_templates` 필드 추가 |
| `crates/kmd-core/src/search.rs` | `filter_contains()` 멀티 토큰 AND 매칭 + 가중 스코어링 |
| `crates/kmd-core/src/history.rs` | `boost_results()` frecency 알고리즘, `parse_hours_ago()` |
| `crates/kmd-core/src/db.rs` | `prune_history()`, DB migration v2 (`executed_at` 인덱스) |
| `src/tui/app.rs` | `classify_web_query`/`extract_batch_urls` 적용, `:prompt`/`:t` prefix 추가 |
| `crates/kmd-desktop/src/app.rs` | 동일 리팩토링 + `:prompt`/`:t` 추가 |
| `src/main.rs` | `Prompt` 서브커맨드 추가 |
| `src/cmd/mod.rs` | `prompt` 모듈 등록 |
| `README.md` | Smart Directory Jump, frecency, v0.3.0 로드맵 업데이트 |

---

## 테스트 결과

- web 모듈: 17개 테스트 (기존 15 + 신규 2)
- prompt 모듈: 7개 테스트
- transform 모듈: 9개 테스트
- search 모듈: 멀티 토큰 테스트 9개 추가 (한글/영문/혼합/Windows/순서무관)
- history 모듈: frecency 테스트 7개 추가 (시간감쇠/파싱실패/CAP)
- db 모듈: pruning 테스트 4개 추가 (감쇠/삭제/임계값/마이그레이션)
- 전체 117개 테스트 통과, workspace 전체 빌드 성공

---

*작성일: 2026-02-17*
