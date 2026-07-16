# LLM 오토파일럿 — 확장 없이 프롬프트 자동 실행 + 이어서 질문

작성: 2026-07-15 (v0.9.5 기준)

## 1. 문제 정의

`@gpt`/`@claude`/`@gemini` 등 LLM 프리픽스는 URL 템플릿(`?q=`)으로 프롬프트를
전달하는데, 서비스마다 처리가 다르다:

| 서비스 | URL `?q=` | 현재 결과 | 원인 |
|---|---|---|---|
| Perplexity, Grok | 지원 | ✅ 자동 실행 | 검색엔진식 URL |
| ChatGPT, Claude | 프리필만 | ⚠ 붙여넣기만, 실행 안 됨 | 2025-04 Tenable 프롬프트 인젝션 보고 후 OpenAI가 외부 네비게이션 자동 제출 차단. Claude는 프리필만 지원 |
| Gemini | 미지원 | ⚠ 클립보드 폴백(수동) | URL 파라미터 네이티브 미지원 |

브라우저 확장을 쓰면 완전 해결되지만, **확장 설치를 늦추고 keymander 단독으로**
최선을 내는 것이 목표(사용자 결정 2026-07-15).

추가 요구: 여러 LLM에 **이어서 동시에** 후속 질문. 현재는 각 창에서 수동 입력.

## 2. 접근 — 데몬 키 주입 오토파일럿

kmd-daemon은 이미 전역 키보드 훅 + SendInput 인프라를 가진다. 이를 재사용해:

1. **단발 자동 실행**: URL을 열고 → **전경창이 기대한 브라우저+타이틀일 때만**
   → 프리필형(gpt/claude)은 Enter, 클립보드형(gemini)은 Ctrl+V→Enter 주입.
2. **이어서 질문(`@@`)**: 각 LLM을 세션 레지스트리(service→HWND)에 기억했다가
   후속 프롬프트를 각 창에 포커스→붙여넣기→Enter로 순차 전달.

### 안전의 핵심 — 타이틀 게이트

키를 아무 창에나 쏘면 사고다. 주입 직전 반드시 검증:

- `GetForegroundWindow()`의 프로세스가 **알려진 브라우저**(chrome/msedge/
  firefox/brave/whale/opera/vivaldi)이고,
- 창 타이틀이 **기대 마커**("ChatGPT"/"Claude"/"Gemini"/…)를 포함할 때만 주입.
- 폴링 타임아웃(기본 8s) 내 조건 불충족 시 **조용히 포기** — gpt/claude는
  프리필, gemini는 클립보드가 남아 사용자가 수동 완료 가능(현행과 동일). 회귀 없음.
- 주입 직전 settle 지연(기본 450ms) 후 **재검증**해서, 로딩 중 포커스가 튄 경우
  차단.

### 기본 off (opt-in)

Enter/Ctrl+V 자동 주입은 잠재적 오작동 여지가 있어 **기본 비활성**.
`[launcher.web] llm_autopilot = true`로 켠다. 꺼져 있으면 현행 URL/클립보드 동작.

## 3. 데이터 모델 (kmd-core)

```rust
// ipc.rs
Request::LlmAutopilot { jobs: Vec<LlmJob> }   // 새 대화 열기 + 주입
Request::LlmFollowup  { prompt: String }      // 기억된 창에 이어서

struct LlmJob {
    service_id: String,          // "chatgpt" — 레지스트리 키
    url: String,                 // 열 URL (프리필 포함)
    prompt: String,              // 클립보드형 주입/폴백용 원문
    method: LlmInject,           // EnterOnly | PasteEnter
    title_markers: Vec<String>,  // 전경창 검증 마커
}
enum LlmInject { EnterOnly, PasteEnter }
```

서비스 메타는 `WebService`에 `automation: Option<LlmAutomation>` 추가:
- `chatgpt`/`claude`: `EnterOnly`, 마커 `["ChatGPT"]`/`["Claude"]`
- `gemini`: `PasteEnter`, 마커 `["Gemini"]`, URL은 프리필 없이 `app`
- `perplexity`/`grok`: 자동화 불필요(URL이 이미 실행) → `automation = None`,
  오토파일럿 잡에서 제외하고 그냥 URL만 연다.

## 4. 데몬 구현 (Windows, `keybind`와 분리된 `autopilot` 모듈)

키보드 훅과 얽히지 않도록 자체 SendInput(Enter, Ctrl+V) + Win32 창 조회를 가진
독립 모듈. 전경창 폴링이 수 초 걸리므로 **전용 스레드**(키 액션 워커와 별개).

```
fresh(jobs):
  for job in jobs (순차 — 전경창은 공유 자원):
    if method == PasteEnter: set_clipboard(job.prompt)
    open_url(job.url)
    hwnd = poll_foreground_until_match(job.title_markers, browser, timeout)
    if hwnd is None: log-skip; continue
    settle(450ms); re-verify(hwnd, markers)
    method == EnterOnly  → send Enter
    method == PasteEnter → send Ctrl+V; settle; send Enter
    registry.insert(job.service_id, hwnd)

followup(prompt):
  for (service_id, hwnd) in registry:
    if !IsWindow(hwnd) || !title_matches: drop; continue
    SetForegroundWindow(hwnd); settle
    set_clipboard(prompt); send Ctrl+V; settle; send Enter
```

- macOS/Linux: 스텁(현행 URL/클립보드 동작 유지). Windows 먼저.
- stuck 방지: 오토파일럿은 modifier를 홀드 상태로 남기지 않음(Ctrl+V는 down+up 원자).

## 5. 프런트엔드 라우팅 (desktop/TUI)

LLM 실행 지점(`launch.rs`)에서:
1. autopilot 켜짐 + 데몬 실행 중 + LLM 잡 있음 → `Request::LlmAutopilot` 전송,
   자동화 대상만 데몬이 처리. 자동화 없는(perplexity/grok) 서비스는 잡에서
   빼고 데스크톱이 그냥 URL로 연다(현행).
2. 아니면 **완전 현행 폴백**(URL + 멀티 클립보드).

`@@ <프롬프트>`(신규 프리픽스) → `Request::LlmFollowup`. 데몬 없거나 레지스트리
비어 있으면 안내만.

## 6. 단계

- P1: 설계(본 문서) + core IPC/메타/`@@` 파서 + 데몬 autopilot(fresh, Windows)
- P2: followup(`@@`) + 세션 레지스트리
- P3: 데스크톱/TUI 라우팅 + 폴백 + config 토글 + 실기기 검증

## 7. 실기기 검증 체크리스트 (Windows, `llm_autopilot = true`)

1. `@gpt 질문` → ChatGPT 탭 열림 + 자동 제출
2. `@claude 질문` → 자동 제출
3. `@gemini 질문` → 붙여넣기 + 자동 제출
4. `@llm 질문`(멀티) → 대상 전부 순차 자동 제출, perplexity/grok도 정상
5. 주입 대기 중 다른 창 클릭 → **엉뚱한 창에 Enter 안 감**(게이트 확인)
6. `@@ 후속질문` → 열려 있던 LLM 창들에 이어서 전달
7. autopilot off → 현행(프리필/클립보드)으로 동작, 회귀 없음
8. 한글 프롬프트 IME 조합 간섭 없음(클립보드 경유라 안전 예상)

## 8. 한계 (정직하게)

- 브라우저 타이틀/포커스 타이밍에 의존 — 콜드 스타트가 느리면 타임아웃 가능
  (게이트가 막아 안전하지만 자동 실행은 실패 → 수동 폴백).
- 서비스가 브랜드 타이틀을 바꾸면 마커 갱신 필요.
- 완전한 해결(임의 타이밍·백그라운드 주입·대화 스레드 정확 타게팅)은 결국
  브라우저 확장(docs 별도) 몫. 본 기능은 "확장 없이 가능한 최선".
