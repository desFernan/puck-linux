# Puck for Linux

> Language: [English](README.md) · **한국어** (here)

> [**desFernan/puck-mac**](https://github.com/desFernan/puck-mac)(Swift/AppKit,
> macOS)의 Linux 포팅입니다. Rust + GTK4, X11.
>
> 플랫폼: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (여기)

### 💬 [디스코드 참여하기](https://discord.gg/nGqtBGP857)

버그 제보, 기능 요청, 빌드 관련 질문, 아니면 그냥 놀러 오고 싶어도 —
[서포트 서버](https://discord.gg/nGqtBGP857)가 가장 빠른 연락 방법입니다. 놀러 오세요!

AI 에이전트이기도 한 Linux 데스크톱 펫입니다. Rust 바이너리 세 개로 구성돼 있어요:

- **`puck-linux`** — 펫 본체: 항상 위에 떠 있는 투명한 캐릭터가 움직이고,
  드래그할 수 있습니다. puck-mac과 같은 아바타 폴더를 읽습니다.
- **`puck-agent`** — 터미널 안의 에이전트: Anthropic API에 붙는 REPL이고,
  `run_shell`은 호출마다 승인을 받습니다.
- **`puck-client`** — 같은 에이전트를 띄우는 최소한의 GTK4 채팅 창. 지금의
  `PuckClient` 대응물이며, 승인은 터미널 프롬프트 대신 예/아니오 대화상자입니다.

셋은 로컬 소켓 브리지(`crates/puck-core/src/bridge.rs`)로 통신합니다: 프런트엔드가 요청을
처리하는 동안 펫에게 `thinking` 클립을, 턴이 끝나면 결과에 따라 `happy` 또는
`sad`를 보여 주라고 알려 줍니다 (아바타에 그 클립이 없으면 `idle`로 대체).
puck-mac의 펫-클라이언트 아키텍처의 첫 조각이고, 아직 메시지는 이것 하나뿐입니다
— 세션 공유나 채팅 내용 전달 같은 건 없습니다. 에이전트 코어는 `crates/puck-core/`에
있습니다.

코드는 크레이트 세 개입니다:

```
crates/puck-core/    에이전트(Anthropic 클라이언트, 도구 루프, 세션)와
                     브리지. 순수 Rust, 데스크톱 의존성 없음.
crates/puck-agent/   터미널 프런트엔드.
crates/puck-linux/   펫과 GTK 채팅 창, 그리고 그 아래의 avatar·motion·
                     emotion·window 모듈.
```

아직 포팅되지 않은 것: Wayland 지원(지금은 X11 전용),
그리고 puck-mac의 진짜 `PuckClient`에 있는 코드 에디터·터미널 패널·워크스페이스.

## 빌드

Rust와 GTK4 개발 헤더(Debian/Ubuntu는 `libgtk-4-dev libx11-dev`, Fedora는
`gtk4-devel` + X11 개발 패키지)가 필요하고, X11 세션에서 동작합니다.

```sh
cargo run --bin puck-linux -- /path/to/avatar-folder   # 펫
cargo run --bin puck-agent                             # 터미널 채팅
cargo run --bin puck-client                            # GTK4 채팅 창
```

`puck-agent`만은 아무것도 필요 없습니다. 자기 크레이트에 든 터미널 프로그램이라
GTK나 X11 없이도 `cargo run -p puck-agent`로 빌드됩니다.

펫은 아바타 폴더를 유일한 인자로 받습니다 — [내 것으로
만들기](#내-것으로-만들기) 참고.

## 테스트

```sh
cargo test                 # 전부
cargo test -p puck-core    # 에이전트 + 브리지만 — GTK도 X11도 Linux도 필요 없음
cargo test -p puck-linux   # 펫: 패키지 파싱, 모션 상태기계, 감정 오버라이드
```

`puck-core`는 순수 Rust라서 그 테스트 — 와이어 포맷, 도구 호출 루프, 실제
HTTP·소켓 왕복 — 는 GTK 개발 헤더가 아예 없는 머신에서도 그대로 돕니다.

브리지 테스트는 임시 경로의 실제 Unix 소켓을 씁니다. `PUCK_BRIDGE_SOCKET`으로
펫·`puck-agent`·`puck-client`를 기본이 아닌 소켓에 붙일 수 있습니다 — 여러 개를
동시에 띄울 때 서로 충돌하지 않게 하는 데 유용합니다.

## 에이전트 프로바이더

Anthropic HTTP API를 직접 호출합니다. 두 프런트엔드 모두 `ANTHROPIC_API_KEY`를
환경 변수에서 읽고, 없으면 현재 디렉터리의 `.env` 파일(한 줄에 `KEY=VALUE`)에서
읽습니다 — puck-mac의 자격 증명 파일과 같은 방식입니다. 기본 모델은 둘 다
`claude-opus-5`이고, `PUCK_AGENT_MODEL`로 바꿀 수 있습니다.

지금 있는 도구는 `run_shell` 하나뿐이고, 에이전트 프로세스와 같은 권한으로
명령을 실행합니다 — 샌드박스도 allowlist도 **없습니다**. 모든 호출은 도구 이름과
정확한 입력을 보여 주고 먼저 묻습니다: `puck-agent`는 `y`/`yes` 프롬프트,
`puck-client`는 예/아니오 대화상자.

## 내 것으로 만들기

아바타는 `manifest.json` 하나와 클립별 PNG가 든 폴더이고, 펫에게 그 폴더를
직접 가리켜 줍니다:

```
my-pet/
    manifest.json
    idle.png  walk.png  fall.png  …
```

```sh
cargo run --bin puck-linux -- ./my-pet
```

### 캐릭터

그림 한 장이면 동작하는 캐릭터입니다 — 반드시 있어야 하는 클립은 `idle`
하나뿐이고, 이 포팅은 `walk`·`fall`·`land`가 있으면 쓰고 없으면 `idle`로
떨어집니다. 배경은 투명하게, 오른쪽을 보게 그리세요. 동작하는 가장 작은
manifest:

```json
{
  "schema_version": 1,
  "name": "my-pet",
  "type": "sprites",
  "hitbox": { "width": 130, "height": 133 },
  "clips": { "idle": "idle" }
}
```

`hitbox`는 그려지는 크기입니다 — 그림 비율과 맞추지 않으면 찌그러져 보입니다.
`emotions`도 읽으며, 에이전트가 일하는 동안 브리지가 바꿔 끼우는 것이 이쪽입니다.

패키지에 문제가 있으면 펫은 뜨지 않고 이유를 stderr에 적습니다 — `idle` 파일이
없거나, manifest가 파싱되지 않거나, 패키지 밖으로 나가는 경로이거나.

패키지 형식(`schema_version: 1`)은 puck-mac이 정의하고 여기서는 그대로
읽습니다. macOS에서 만든 아바타 폴더가 그대로 들어옵니다. 전체 필드 설명 —
`clips`, `emotions`, `sounds`, `hitbox`, `bounce_intensity`와 각각의 기본값 —
은 [puck-mac README](https://github.com/desFernan/puck-mac#a-character)에
있고, 이 포팅은 위의 부분집합만 읽고 나머지는 무시합니다.

## 커뮤니티

질문, 버그 제보, 기능 아이디어, 아니면 그냥 직접 만든 아바타를 자랑하고 싶어도 —
**[디스코드](https://discord.gg/nGqtBGP857)**에서 만나요.

## 라이선스

소스는 MIT입니다 — [LICENSE](LICENSE). 옆에 있는 **그림·아이콘·폰트·오디오는
아닙니다**: 이유는 [LICENSE-ASSETS.md](LICENSE-ASSETS.md)에 적어두었습니다.
