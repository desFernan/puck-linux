# Puck for Linux

> Language: [English](README.md) · **한국어** (here)

> Puck은 현재 macOS에 존재합니다 — 전체 앱은
> [desFernan/puck-mac](https://github.com/desFernan/puck-mac)을 보세요. 이
> 저장소는 Linux 포팅이며, 현재는 펫 오버레이만 있습니다 (에이전트는 아직 —
> 아래 상태 참고).
>
> 플랫폼: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (여기)

### 💬 [디스코드 참여하기](https://discord.gg/ePBZVnwSYE)

버그 제보, 기능 요청, 빌드 관련 질문, 아니면 그냥 놀러 오고 싶어도 —
[서포트 서버](https://discord.gg/ePBZVnwSYE)가 가장 빠른 연락 방법입니다. 놀러 오세요!

## 상태

펫 오버레이 MVP가 여기 있습니다: 항상 위에 떠 있고, 투명하며, 드래그해서
움직일 수 있는 애니메이션 캐릭터로, [puck-mac](https://github.com/desFernan/puck-mac)과
같은 아바타 폴더 포맷을 사용합니다. 에이전트 기능은 아직 없습니다 — 범위는
[`docs/superpowers/specs/2026-08-24-linux-pet-mvp-design.md`](docs/superpowers/specs/2026-08-24-linux-pet-mvp-design.md)를
참고하세요. 에이전트 코어, `PuckClient`에 해당하는 창, 그리고 둘 사이의 소켓
브릿지는 이후 작업입니다.

### 빌드 및 실행

Rust와 GTK4 개발 헤더(Debian/Ubuntu는 `libgtk-4-dev libx11-dev`, Fedora는
`gtk4-devel` + X11 개발 패키지)가 필요하고, X11 세션에서 동작합니다 —
Wayland는 아직 지원하지 않습니다.

```sh
cargo run -- /path/to/avatar-folder
```

아바타 폴더에는 `manifest.json`과 클립별 PNG가 필요합니다 — 매니페스트
스키마는 [puck-mac README](https://github.com/desFernan/puck-mac/blob/main/README.ko.md#캐릭터)를
참고하세요. 이 포팅은 `schema_version`, `name`, `type`, `hitbox`, `clips`를
읽으며, `idle`만 필수이고 `walk`/`fall`/`land`는 있으면 사용하고 없으면
`idle`로 대체합니다.

### 테스트

```sh
cargo test --bin puck-linux
```

(`cargo test --lib`이 아닙니다 — 이 크레이트는 bin 전용이라 라이브러리
타깃이 없습니다.)

### 에이전트 프로바이더

이 포팅에서는 아직 만들어지지 않았습니다. macOS는 채팅을 위해 Anthropic
또는 OpenAI API와 직접 통신하고, `code_editor` 도구를 위해 `node` 아래에서
벤더 ACP 에이전트를 돌립니다 — 이 계층은 이 포팅 차례가 되면 최소한의
변경으로 포팅될 예정입니다.

### 내 것으로 만들기

아바타 패키지 포맷(`schema_version: 1`, `manifest.json` + 클립 PNG)은
puck-mac이 정의하며 여기서도 그대로 읽습니다 — macOS에서 만든 아바타
폴더가, Windows에서 지금 그렇듯, Linux에도 수정 없이 그대로 들어갑니다.
필드 설명:
[puck-mac README](https://github.com/desFernan/puck-mac/blob/main/README.ko.md#캐릭터).

## 커뮤니티

Linux 포팅 계획에 힘을 보태고 싶거나, 그냥 진행 상황이 궁금하다면 —
**[디스코드](https://discord.gg/ePBZVnwSYE)**로 오세요.
