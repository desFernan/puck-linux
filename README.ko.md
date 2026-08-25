# Puck for Linux

> Language: [English](README.md) · **한국어** (here)

> Puck은 현재 macOS에 존재합니다 — 전체 앱은
> [desFernan/puck-mac](https://github.com/desFernan/puck-mac)을 보세요. 이
> 저장소는 Linux 포팅이며, 펫 오버레이와 에이전트 코어, 최소 기능의
> `PuckClient` 대응 GUI가 여기 있습니다. 펫과 클라이언트 사이의 소켓
> 브릿지는 아직 이후 작업입니다 — 아래 상태 참고.
>
> 플랫폼: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (여기)

### 💬 [디스코드 참여하기](https://discord.gg/ePBZVnwSYE)

버그 제보, 기능 요청, 빌드 관련 질문, 아니면 그냥 놀러 오고 싶어도 —
[서포트 서버](https://discord.gg/ePBZVnwSYE)가 가장 빠른 연락 방법입니다. 놀러 오세요!

## 상태

지금까지 세 조각이 있습니다:

- **펫 오버레이** (`puck-linux`): 항상 위에 떠 있고, 투명하며, 드래그해서
  움직일 수 있는 애니메이션 캐릭터로, [puck-mac](https://github.com/desFernan/puck-mac)과
  같은 아바타 폴더 포맷을 사용합니다.
- **에이전트 코어** (`src/agent/`): Anthropic API와 직접 통신하며, 호출마다
  승인이 필요한 `run_shell` 도구를 가지고 있습니다.
- **에이전트용 프런트엔드 둘**: 터미널 REPL인 `puck-agent`와, 지금의
  `PuckClient` 대응인 최소 기능 GTK4 채팅 창 `puck-client` — 승인은 터미널
  프롬프트 대신 Yes/No 다이얼로그로 뜹니다. 둘 다 puck-mac의 진짜
  `PuckClient`에 있는 코드 에디터, 터미널 패널, 워크스페이스는 없습니다.

이들은 아직 펫 오버레이와 통신하지 않습니다 — 소켓 브릿지도, 공유
프로세스도 없습니다. 아직 이후 작업입니다.

### 빌드 및 실행 — 펫 오버레이

Rust와 GTK4 개발 헤더(Debian/Ubuntu는 `libgtk-4-dev libx11-dev`, Fedora는
`gtk4-devel` + X11 개발 패키지)가 필요하고, X11 세션에서 동작합니다 —
Wayland는 아직 지원하지 않습니다.

```sh
cargo run --bin puck-linux -- /path/to/avatar-folder
```

아바타 폴더에는 `manifest.json`과 클립별 PNG가 필요합니다 — 매니페스트
스키마는 [puck-mac README](https://github.com/desFernan/puck-mac/blob/main/README.ko.md#캐릭터)를
참고하세요. 이 포팅은 `schema_version`, `name`, `type`, `hitbox`, `clips`를
읽으며, `idle`만 필수이고 `walk`/`fall`/`land`는 있으면 사용하고 없으면
`idle`로 대체합니다.

### 빌드 및 실행 — 에이전트

```sh
export ANTHROPIC_API_KEY=sk-ant-...   # 또는 .env 파일에 넣기
cargo run --bin puck-agent    # 터미널 채팅
cargo run --bin puck-client   # GTK4 채팅 창 (X11 세션 필요)
```

둘 다 `ANTHROPIC_API_KEY`를 환경 변수에서 읽고, 환경 변수가 없으면 현재
디렉터리의 `.env` 파일(줄마다 `KEY=VALUE`)에서 읽습니다 — puck-mac의 자격
증명 파일과 동일한 방식입니다. 둘 다 기본 모델은 `claude-opus-5`이며,
`PUCK_AGENT_MODEL`로 바꿀 수 있습니다.

지금 있는 도구는 `run_shell` 하나뿐이며, 에이전트 프로세스와 동일한 권한으로
셸 명령을 실행합니다 — 샌드박스나 허용목록이 **없습니다**. 모든 호출은
실행되기 전에 승인이 필요하고 도구 이름과 정확한 입력을 보여줍니다 —
`puck-agent`는 `y`/`yes` 프롬프트로, `puck-client`는 Yes/No 다이얼로그로.

### 테스트

```sh
cargo test --bin puck-linux   # 펫 오버레이: 파싱, 애니메이션/물리 상태 머신
cargo test --lib              # 에이전트: 와이어 포맷, 도구 호출 루프, 로컬 목 서버 대상 실제 HTTP 왕복
```

(`cargo test`를 인자 없이 실행하면 위 둘과 `puck-agent` 바이너리 자체의,
현재는 비어 있는, 테스트 타깃까지 모두 돕니다.)

### 내 것으로 만들기

아바타 패키지 포맷(`schema_version: 1`, `manifest.json` + 클립 PNG)은
puck-mac이 정의하며 여기서도 그대로 읽습니다 — macOS에서 만든 아바타
폴더가, Windows에서 지금 그렇듯, Linux에도 수정 없이 그대로 들어갑니다.
필드 설명:
[puck-mac README](https://github.com/desFernan/puck-mac/blob/main/README.ko.md#캐릭터).

## 커뮤니티

Linux 포팅 계획에 힘을 보태고 싶거나, 그냥 진행 상황이 궁금하다면 —
**[디스코드](https://discord.gg/ePBZVnwSYE)**로 오세요.
