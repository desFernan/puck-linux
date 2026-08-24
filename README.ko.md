# Puck for Linux

> Language: [English](README.md) · **한국어** (here)

> 아직 시작 전입니다. Puck은 현재 macOS에만 존재해요 — 실제 물건은
> [desFernan/puck-mac](https://github.com/desFernan/puck-mac)을 보세요. 이
> 저장소는 앞으로 있을 Linux 포팅을 위한 자리표시자이며, 아래 내용은 만들어질
> 형태를 미리 적어둔 것이지 실제 구현된 코드가 아닙니다.
>
> 플랫폼: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (여기)

### 💬 [디스코드 참여하기](https://discord.gg/ePBZVnwSYE)

버그 제보, 기능 요청, 빌드 관련 질문, 아니면 그냥 놀러 오고 싶어도 —
[서포트 서버](https://discord.gg/ePBZVnwSYE)가 가장 빠른 연락 방법입니다. 놀러 오세요!

## 상태

아직 아무것도 없습니다 — 코드도, 계획 문서도 없어요.
[puck-windows](https://github.com/desFernan/puck-windows)는 코드가 붙기 전에
`docs/porting-design.md`부터 써서 C# / .NET 8 + WPF로 스택을 정했는데, Linux
포팅도 같은 순서로 시작될 겁니다: 스택 정하기 → macOS 모듈을 대응시키기 →
단계 계획 쓰기 → 포팅.

## Linux 포팅이 생기면 이런 모습일 것

[puck-mac](https://github.com/desFernan/puck-mac),
[puck-windows](https://github.com/desFernan/puck-windows)와 같은 형태로 —
코드가 붙기 전까지는 예상도입니다:

### 빌드

다른 두 포팅과 같은 계약을 가진 `pet-app/scripts/` 빌드 스크립트. 스택
(GTK? Qt? 아니면 전혀 다른 것?)은 아직 정해지지 않았습니다.

### 테스트

실패 시 nonzero로 종료하는 무인 테스트 스크립트 — macOS의
`pet-app/scripts/test.sh`, Windows의 `pet-app/scripts/test.ps1`과 같은 계약.

### 에이전트 프로바이더

macOS와 같은 설계: 일반 채팅은 Anthropic 또는 OpenAI API와 직접 통신하고,
`code_editor` 도구는 `node` 아래에서 벤더 ACP 에이전트를 돌립니다. Linux
전용 로직이 아니라 최소한의 변경으로 포팅될 계층입니다.

### 내 것으로 만들기

아바타 패키지 포맷(`schema_version: 1`, `manifest.json` + 클립 PNG)은
puck-mac이 정의하며 모든 포팅에서 그대로 읽도록 설계돼 있습니다 — macOS에서
만든 아바타 폴더가, Windows에서 지금 그렇듯, Linux에도 수정 없이 그대로
들어갈 예정입니다. 필드 설명:
[puck-mac README](https://github.com/desFernan/puck-mac/blob/main/README.ko.md#캐릭터).

## 커뮤니티

Linux 포팅 계획에 힘을 보태고 싶거나, 그냥 진행 상황이 궁금하다면 —
**[디스코드](https://discord.gg/ePBZVnwSYE)**로 오세요.
