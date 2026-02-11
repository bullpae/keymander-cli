# Theming Specification

## 1. 개요

keymander TUI의 색상과 스타일을 커스터마이즈하는 시스템.
현재는 코드 내장 테마를 사용하며, 향후 TOML 기반 외부 테마 파일을 지원할 예정.

---

## 2. 현재 구현 (v0.2)

### 2.1 Theme 구조체

```mermaid
classDiagram
    class Theme {
        +Color input_fg
        +Color input_border
        +Color list_selected_bg
        +Color list_selected_fg
        +Color list_normal_fg
        +Color kind_tag_fg
        +Color path_fg
        +Color status_fg
        +Color status_bg
        +Color header_fg
        +input_style() Style
        +input_border_style() Style
        +list_selected_style() Style
        +list_normal_style() Style
        +kind_tag_style() Style
        +path_style() Style
        +status_style() Style
        +header_style() Style
        +default_theme() Theme
    }
```

### 2.2 테마가 적용되는 영역

```mermaid
block-beta
    columns 1
    block:headerArea:1
        HeaderText["header_fg (Cyan Bold): kmd v0.2.0 | 1234 items"]
    end
    block:inputArea:1
        InputBorder["input_border (Cyan): 테두리"]
        InputText["input_fg (White): > fire_"]
    end
    block:contentArea:1
        columns 2
        block:listArea:1
            Selected["list_selected_bg/fg (DarkGray/White Bold): ▸ Firefox"]
            Normal["list_normal_fg (Gray): FileZilla"]
            KindTag["kind_tag_fg (DarkGray): [App]"]
            PathText["path_fg (DarkGray): /usr/bin/firefox"]
        end
        block:previewArea:1
            PreviewContent["list_normal_fg (Gray): Name, Type, Path..."]
        end
    end
    block:statusArea:1
        StatusText["status_fg/bg (DarkGray): [fuzzy] 4 results | ..."]
    end
```

### 2.3 기본 테마 값

| 요소 | 색상 | 수정자 |
|------|------|--------|
| input_fg | White | — |
| input_border | Cyan | — |
| list_selected_bg | DarkGray | — |
| list_selected_fg | White | Bold |
| list_normal_fg | Gray | — |
| kind_tag_fg | DarkGray | — |
| path_fg | DarkGray | — |
| status_fg | DarkGray | — |
| status_bg | Reset | — |
| header_fg | Cyan | Bold |

---

## 3. 향후 계획 (v0.3+)

### 3.1 TOML 테마 파일

```toml
# ~/.config/kmd/themes/catppuccin-mocha.toml

[meta]
name = "Catppuccin Mocha"
author = "catppuccin"

[colors]
input_fg = "#CDD6F4"        # Text
input_border = "#89B4FA"     # Blue
list_selected_bg = "#45475A" # Surface1
list_selected_fg = "#CDD6F4" # Text
list_normal_fg = "#BAC2DE"   # Subtext1
kind_tag_fg = "#6C7086"      # Overlay0
path_fg = "#6C7086"          # Overlay0
status_fg = "#585B70"        # Surface2
status_bg = "#1E1E2E"        # Base
header_fg = "#89B4FA"        # Blue
```

### 3.2 테마 로드 흐름

```mermaid
flowchart TD
    Start["TUI 시작"] --> ReadConfig["config.toml<br/>general.theme 읽기"]
    ReadConfig --> CheckTheme{"theme == 'default'?"}
    CheckTheme -- Yes --> DefaultTheme["하드코딩된 기본 테마"]
    CheckTheme -- No --> FindFile["themes/{name}.toml 탐색"]
    FindFile --> Exists{"파일 존재?"}
    Exists -- Yes --> ParseTOML["TOML 파싱"]
    ParseTOML --> ValidColors{"색상값 유효?"}
    ValidColors -- Yes --> ApplyTheme["테마 적용"]
    ValidColors -- No --> Fallback["기본 테마로 폴백 + 경고"]
    Exists -- No --> Fallback
    DefaultTheme --> Render["렌더링"]
    ApplyTheme --> Render
    Fallback --> Render
```

### 3.3 예정 테마

| 이름 | 설명 | 계열 |
|------|------|------|
| default | 터미널 기본 색상 활용 | 범용 |
| catppuccin-mocha | 따뜻한 다크 테마 | Dark |
| catppuccin-latte | 밝은 라이트 테마 | Light |
| nord | 차가운 파란 계열 | Dark |
| tokyo-night | VS Code 인기 테마 포트 | Dark |
| gruvbox | 레트로 컬러 | Dark |
| solarized-dark | 클래식 다크 테마 | Dark |

### 3.4 색상 시스템

```mermaid
flowchart TD
    Detect["터미널 색상 지원 감지"] --> Check1{"COLORTERM=truecolor?"}
    Check1 -- Yes --> TrueColor["24-bit True Color<br/>Color::Rgb(r, g, b)<br/>1677만 색"]
    Check1 -- No --> Check2{"TERM=*-256color?"}
    Check2 -- Yes --> Color256["256색 팔레트<br/>Color::Indexed(n)<br/>256 색"]
    Check2 -- No --> Color16["16색 기본<br/>Color::Red 등<br/>16 색"]

    TrueColor --> Render["테마 색상 렌더링"]
    Color256 --> Downgrade256["24bit → 가장 가까운 256색 매핑"]
    Downgrade256 --> Render
    Color16 --> Downgrade16["256색 → 가장 가까운 16색 매핑"]
    Downgrade16 --> Render
```

### 3.5 테마 핫 리로드 (향후)

```mermaid
sequenceDiagram
    participant User
    participant FileWatcher as File Watcher
    participant ThemeLoader as Theme Loader
    participant TUI as TUI Renderer

    User->>User: themes/nord.toml 수정 저장
    FileWatcher->>ThemeLoader: 파일 변경 감지
    ThemeLoader->>ThemeLoader: TOML 재파싱
    ThemeLoader->>TUI: 새 Theme 적용
    TUI->>TUI: 다음 프레임에서 반영
    TUI-->>User: 실시간 색상 변경
```

---

## 4. 아이콘 시스템

### 4.1 현재 구현 (Unicode Emoji)

Nerd Font에 의존하지 않고 Unicode 이모지 사용:

| 확장자/종류 | 아이콘 | | 확장자/종류 | 아이콘 |
|-------------|--------|-|-------------|--------|
| .rs | 🦀 | | .json/.yaml/.toml | 📋 |
| .py | 🐍 | | 이미지 | 🖼️ |
| .js/.ts | 📜 | | 음악 | 🎵 |
| .go | 🔵 | | 동영상 | 🎬 |
| .java/.kt | ☕ | | 압축파일 | 📦 |
| .c/.cpp | ⚙️ | | 디렉토리 | 📁 |
| .sh/.bash | 🐚 | | 실행파일 | ⚡ |
| .md/.txt/.doc | 📝 | | 히스토리 | 🕒 |
| .pdf | 📕 | | 기타 | 📄 |

### 4.2 향후: 아이콘 스타일 선택

```mermaid
flowchart LR
    Config["config.toml<br/>icon_style"] --> Emoji["emoji (기본)<br/>Unicode Emoji<br/>모든 터미널"]
    Config --> NerdFont["nerd-font<br/>Nerd Font 글리프<br/>NF 설치 필요"]
    Config --> ASCII["ascii<br/>순수 텍스트<br/>SSH / 레거시 터미널"]
```

config.toml:
```toml
[general]
icon_style = "emoji"      # 기본값
# icon_style = "nerd-font"
# icon_style = "ascii"
```
