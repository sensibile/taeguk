# 코드 그래프 도구 작업 계약

2026-10-28. 상태: Phase 0 확정, Phase 1 1계층 설계·검증 계획 작성. 목적 가설과 CG-R01~R06의 의미는 같은 날 BOSS가 “권장안대로 확정”으로 확인했다. 외부 도구 설치와 시험 프로젝트 적용은 각각 별도 확인을 받는다.

## 요청과 결정 경과

- BOSS 원래 표현: “태극의 코드 그래프 도구를 도입하고 싶어”, 이후 “2계층안으로 진행”.
- 같은 날 BOSS가 고른 항목: 목적은 변경 영향 범위 파악·변경 부담 진단·에이전트 코드 탐색 세 가지, 방식은 기존 도구 연동, 대상은 시험 프로젝트 언어와 태극 자체.
- 2계층안: 증거용 결정적 그래프(1계층)와 에이전트 탐색용 LSP 도구(2계층)를 분리한다.

## 목적 가설

변경 지점에서 의존·호출 관계를 실제 도구 출력으로 확인할 수 있으면 재검증 대상 선택과 변경 부담 진단이 추정이 아니라 관측에 기반하고, 에이전트가 시험 프로젝트 구조를 파악하는 반복 탐색이 줄어든다. 도구 설치나 그래프 생성 자체를 목적 달성으로 보지 않는다. 효과는 Phase 4에서 실제 작업 사례로 관측한다.

## 대상

| 저장소 | 언어 | 확인 근거 |
| --- | --- | --- |
| hoplites | Python | `git ls-files` 확장자 집계 (2026-10-28) |
| thesmothetai | Go | 같음 |
| acropolis/akashic | Rust (Elixir·Python 일부) | 같음 |
| acropolis/axiom, atheum | Elixir (Erlang 일부) | 같음 |
| taeguk | Elixir, Rust, Python | 같음 |

로컬 도구: go 1.27.3, Elixir 1.20.4/OTP 29, cargo, rust-analyzer, python3 3.13 확인. 이는 설치 사실이며 각 분석 명령이 대상 저장소에서 동작한다는 증거가 아니다.

## 요구사항 초안

| ID | 기대 결과 | 수용 사례 (대표 / 실패 / 경계) | 미결정 |
| --- | --- | --- | --- |
| CG-R01 | 1계층: 대상 언어 4종의 모듈·패키지 의존 그래프를 언어 공식 도구 또는 표준 라이브러리 분석으로 생성한다 | 각 시험 프로젝트에서 생성 성공 / 컴파일·의존 해석 실패 시 FAIL과 원인을 반환하고 부분 그래프를 완전한 것으로 표시하지 않음 / 동적 import·매크로·리플렉션처럼 정적으로 해석되지 않는 관계는 누락 가능으로 표시 | 확정: 모듈·패키지 단위 |
| CG-R02 | 1계층 결과를 언어와 무관한 공통 JSON으로 정규화하고 대상 commit, 도구 이름·버전, 실행 명령, 생성 시각을 함께 기록한다 | 같은 commit·도구 버전에서 재실행 시 동일 그래프 / 작업 트리에 미커밋 변경이 있으면 그 사실을 기록 / 도구 버전이 다르면 이전 결과를 현재 근거로 재사용하지 않음 | 없음 |
| CG-R03 | 변경 파일 또는 심볼 목록을 입력하면 그 변경에 직접·간접으로 의존하는 대상을 반환한다 | 알려진 의존 관계를 가진 fixture에서 기대 영향 집합과 일치 / 그래프에 없는 관계를 “영향 없음”으로 단정하지 않음 / 순환 의존에서 종료 | 추적 깊이 기본값은 Phase 1 설계에서 정함 |
| CG-R04 | 영향 결과를 `taeguk-plan-cross-verification`의 검사 선택과 `taeguk-inspect-changeability`의 관측 근거로 사용할 수 있게 스킬 지침을 갱신한다 | 스킬 문서가 그래프 결과의 한계(정적 분석 누락, 런타임 경로 미포함)를 함께 명시 / 그래프 결과만으로 PASS를 판정하지 않음 | 없음 |
| CG-R05 | 2계층: 에이전트가 시험 프로젝트에서 심볼 정의·참조를 질의할 수 있는 LSP 기반 탐색 도구를 선택적으로 설치한다 | 설치 후 언어별 참조 질의 성공 / 지원하지 않는 언어는 미지원으로 보고 / 기존 Codex·Claude 설정과 소유하지 않은 설치 파일 보존 | Serena의 Go 지원 여부(공식 문서 미확인). 설치는 프로젝트별 수동으로 확정 |
| CG-R06 | 2계층 결과는 탐색 보조로만 쓰고 완료 판정 근거로 쓰지 않는다 | 스킬·지침에 이 구분이 명시됨 | 없음 |

## 범위와 제약

- 1계층 정규화·영향 질의는 태극 Rust CLI(`tools/taeguk-cli`)에 서브커맨드로 추가한다. 두 번째 백엔드 스택을 추가하지 않는다.
- 언어별 분석기 후보: Elixir `mix xref graph`, Go `go list -deps -json`, Rust `cargo metadata`와 crate 내부 `mod`·`use` 해석, Python 표준 라이브러리 `ast`. 최종 선택은 Phase 1에서 시험 프로젝트 실측 후 확정한다.
- 2계층 후보는 Serena(LSP 기반 MCP). tree-sitter 기반 코드 그래프 MCP 후보들은 성숙도가 낮고 Elixir 지원을 확인하지 못해 이번 범위에서 제외한다.
- 외부 도구 설치, 시험 프로젝트에 대한 설치 적용과 commit은 각각 BOSS 확인 후 진행한다. 운영 배포·DB migration·외부 메시징은 범위 밖이다.
- 상태 API 연동(그래프 결과를 관측으로 등록)은 이번 범위에서 제외하고, 필요가 관측되면 별도 작업으로 다룬다.

## 적용 규칙

`docs/rules/registry.toml` 기준 REQ-001 1.1, REQ-006 1.0, VER-003 1.0, VER-004 1.1, DEV-002 1.1, REL-005 1.0. 작업 단계는 `codex/skills/taeguk-work-contract/references/phases.md`를 따른다. 태극 저장소 작업이므로 시험 프로젝트용 의도·요구사항 기록 의무(REQ-007)는 적용하지 않고 이 문서를 작업 계약으로 쓴다.

## Phase 1 실측 (2026-10-28)

시험 프로젝트에서 분석기 후보를 실행했다. 출력은 세션 scratchpad에 두었고 시험 프로젝트의 `git status`는 실행 전후 같았다.

| 대상 | 명령 | 결과 | 설계에 반영할 점 |
| --- | --- | --- | --- |
| thesmothetai (Go) | `go list -deps -json ./...` | 성공 0.5초, 패키지 234개 중 main module 11개 | main module 패키지만 노드로 쓰고 표준 라이브러리·외부 모듈은 제외 |
| taeguk `apps/taeguk` (Elixir) | 기존 `_build/dev` 사본을 `MIX_BUILD_PATH`로 지정한 `mix xref graph --format dot` | 성공 0.8초, 파일 11개, edge 13개, `(compile)` 표식 포함 | 원래 `_build`를 건드리지 않고 사본에서 실행 가능 |
| acropolis/axiom (Elixir) | 같은 방식 | 성공, 프로젝트 파일 5개 edge 10개. `vendor/` 의존성은 포함되지 않음 | 의존성 코드는 그래프 밖. 결과가 작아도 정상일 수 있음 |
| acropolis/akashic (Rust) | `cargo metadata --offline --locked` | FAIL: `rocksdb`를 crates.io index에서 찾지 못함 | 의존성 해석이 필요 없는 `--no-deps`를 쓴다 |
| acropolis/akashic (Rust) | `cargo metadata --no-deps --offline` | 성공, workspace crate 2개(`akashic-engine -> akashic-core`), crates 아래 `.rs` 26개 | crate 단위만으로는 너무 거칠다. crate 내부 모듈 해석이 필요 |
| hoplites (Python) | `python3 -I`로 실행한 `ast` 기반 시험 스크립트 | 성공, 파일 49개, edge 73개, 상대 import·동적 import 0건 | 이름 해석이 모호한 경우를 별도 gap으로 남겨야 함 |

acropolis/akashic의 `elixir/`처럼 `_build`가 없는 Elixir 프로젝트는 이번 실측 범위에 없다. 설계상 빌드 없이는 FAIL로 반환한다.

## Phase 1 설계 (1계층)

### 명령

- `taeguk graph build ROOT --out GRAPH.json [--lang elixir|go|rust|python ...]`: 언어를 지정하지 않으면 `mix.exs`, `go.mod`, `Cargo.toml`, tracked `.py` 존재로 감지한다.
- `taeguk graph impact GRAPH.json ROOT (--changed PATH ... | --git-diff BASE)`: 변경 파일에서 역방향 edge를 따라 영향 노드를 찾는다.

### 노드와 edge

- 노드 단위는 언어별 도구가 안정적으로 주는 단위를 쓴다. Elixir·Python·Rust는 source 파일, Go는 패키지(소속 파일 목록 포함)다. 변경 파일은 소속 노드로 대응시킨다.
- edge는 “from이 to에 의존한다”는 방향이다. Elixir는 `compile`·`export`·`runtime` 구분을 `kind`로 보존한다.

### 언어별 분석기

| 언어 | 방법 | 실행 조건과 한계 |
| --- | --- | --- |
| Elixir | 기존 `_build/<env>`를 임시 디렉터리로 복사하고 `MIX_BUILD_PATH`로 지정해 `mix xref graph --format dot` 실행 | 대상의 `_build`·`deps`를 만들거나 바꾸지 않는다. `_build`가 없으면 compile·deps.get을 하지 않고 FAIL. 매크로가 런타임에 만드는 호출은 누락 가능 |
| Go | `go list -e -deps -json ./...` | main module 패키지만 노드. 패키지 `Error`·`DepsErrors`가 있으면 FAIL. build tag·`go:generate` 산출물은 현재 환경 기준 |
| Rust | `cargo metadata --no-deps --offline`으로 workspace crate·target·path 의존을 얻고, `syn`으로 각 target의 `mod` 선언과 `use crate::`·`super::`·`self::`·workspace crate 경로를 파일 노드로 해석 | `cfg` 분기는 모두 포함, `#[path]` 지원, 매크로가 만드는 `mod`·`use`와 glob import의 세부 대상은 gap으로 기록. CLI에 `syn` 의존을 추가한다 |
| Python | CLI에 내장한 표준 라이브러리 전용 스크립트를 `python3 -I`로 실행해 `ast`로 import 수집 | tracked `.py`만. 패키지 root는 `pyproject.toml`의 설정, `src/` 배치, 저장소 최상위 순으로 정한다. 상대 import는 패키지 기준으로 해석하고, 여러 파일에 대응되는 이름은 edge를 만들지 않고 gap으로 기록. `importlib`·`__import__`는 gap |

### 결과 형식

```json
{
  "schema": "taeguk.code-graph/1",
  "root": "...", "commit": "<40자리 SHA>", "dirty": false,
  "generated_at": "...",
  "status": "PASS | FAIL",
  "analyzers": [{"lang": "go", "tool": "go", "version": "go1.27.3", "command": ["go", "list", "..."], "exit_code": 0, "status": "PASS", "error": null}],
  "nodes": [{"id": "go:example.com/m/pkg", "lang": "go", "files": ["pkg/a.go"]}],
  "edges": [{"from": "...", "to": "...", "kind": "import"}],
  "gaps": [{"lang": "python", "kind": "ambiguous_import", "file": "...", "detail": "..."}],
  "digest": "sha256:..."
}
```

- `status`는 모든 분석기가 PASS일 때만 PASS다. 하나라도 실패하면 FAIL이고 그 언어의 노드·edge는 비운다. 부분 결과를 완전한 그래프로 표시하지 않는다(CG-R01).
- nodes·edges·gaps는 정렬해 출력한다. `digest`는 `generated_at`을 제외한 commit·dirty·analyzers(버전·명령)·nodes·edges·gaps의 정규화 JSON hash다. 같은 commit·도구 버전의 재실행은 같은 digest를 내야 한다(CG-R02).
- `dirty`가 true면 tracked 파일의 미커밋 변경 경로를 함께 기록한다.

### 영향 질의

- 역방향 edge 전이 폐쇄를 끝까지 따른다. 기본 깊이 제한은 두지 않고 각 노드에 최단 거리를 붙인다. `--depth N`으로 제한하면 적용 깊이를 결과에 기록한다. 순환은 방문 집합으로 종료한다.
- 그래프 `status`가 FAIL이거나, 그래프 commit이 현재 `ROOT`의 HEAD와 다르거나, 그래프 생성 때 dirty였거나, 변경 파일이 어느 노드에도 속하지 않으면 결과 `status`를 UNKNOWN으로 하고 이유를 남긴다. 영향 목록은 참고로 함께 낸다.
- 결과에는 항상 `limits`를 넣는다: 정적 의존만 포함, 런타임·설정·데이터 경로 미포함, edge 부재는 영향 없음의 증거가 아님, 해당 언어 gap 목록.

### 스킬 연결 (CG-R04, CG-R06)

- `taeguk-plan-cross-verification/references/check-selection.md`: 재검증 범위 선택 근거로 `taeguk graph impact` 결과를 쓸 수 있다. 결과만으로 검사를 생략하거나 PASS를 판정하지 않는다.
- `taeguk-inspect-changeability`: 반복 동반 수정 관측에 그래프의 의존 방향·순환을 근거로 덧붙일 수 있다. 수치 복잡도 gate로 쓰지 않는다.
- 두 문서에 2계층 LSP 탐색 결과는 탐색 보조이며 완료 근거가 아니라고 적는다.

### CI와 도구 요구

- Elixir·Go 분석기 검사는 해당 toolchain이 필요하다. 현재 CI `rust` job에는 Go·Elixir가 없으므로 `actions/setup-go`와 `erlef/setup-beam`을 추가하고 `TAEGUK_REQUIRE_GRAPH_TOOLS=1`을 둔다. 이 변수가 있으면 toolchain 부재를 실패로 처리하고, 없으면 해당 테스트를 skip하되 출력에 NOT_RUN을 남긴다. workflow 수정은 위임 범위의 “자율 처리 후 보고”다.
- 1계층은 이미 설치된 go·mix·cargo·python3만 쓴다. 새 외부 도구 설치는 없다. `syn` crate 추가는 의존성 변경이므로 보안 리뷰 대상이다.

## 검증 계획

기대값은 fixture를 만들 때 사람이 정한 의존 관계로 적고 구현 출력에서 역산하지 않는다. 모두 NOT_RUN으로 시작한다.

| ID | 요구 | 검사 | oracle |
| --- | --- | --- | --- |
| CG-V01 | CG-R01 | 언어별 fixture(`tools/taeguk-cli/tests/fixtures/graph/<lang>/`)의 그래프를 수작업 `expected-edges.txt`와 집합 비교. 각 fixture에 순환 1개, 정적으로 해석 불가능한 사례 1개 포함 | 수작업 edge 목록, 해석 불가 사례가 gap으로 나오는지 |
| CG-V02 | CG-R01 실패 | 깨진 `go.mod`, `_build` 없는 Elixir, 구문 오류 Python, 없는 `mod` 파일 Rust에서 `status: FAIL`과 원인 | 사례별 기대 status·원인 문자열 |
| CG-V03 | CG-R01 실측 | hoplites·thesmothetai·acropolis(axiom, akashic)·taeguk에서 build 실행, 실행 전후 대상 `git status`와 `_build` 수정 시각 비교, 무작위 edge 5개를 원본 코드로 수동 대조 | 수동 대조 결과, 상태 불변 |
| CG-V04 | CG-R02 | 같은 commit 재실행 digest 일치, 파일 수정 후 `dirty`와 경로 기록, 기록된 도구 버전과 `--version` 출력 대조 | digest 동일성, 독립 명령 출력 |
| CG-V05 | CG-R03 | fixture에서 변경 파일별 수작업 기대 영향 집합과 비교, 순환 종료, `--depth` 기록, stale·dirty·FAIL 그래프에서 UNKNOWN | 수작업 영향 집합, 사례별 기대 status |
| CG-V06 | CG-R04, CG-R06 | 변경 스킬을 system skill-creator validator로 검사, 한계 문구 존재 확인, installer dry-run으로 설치본 반영 확인 | validator 결과, 문구 대조 |
| CG-V07 | 회귀 | AGENTS.md의 전체 검증 명령(DB 연결 state 테스트 포함) | 각 명령 exit code |
| CG-V08 | 리뷰 | push 전 로컬 Codex 교차·보안 리뷰(`syn` 의존 추가, 외부 명령 실행 인자 처리 포함) | P0·P1 0건 |

2계층(CG-R05~R06)의 검증 계획은 1계층 전달 후 별도로 작성한다.

## BOSS 확정 (2026-10-28)

Claude Code 세션에서 BOSS가 아래 네 항목의 권장안에 “권장안대로 확정”이라고 답했다.

1. 목적 가설과 CG-R01~R06의 의미를 위 표대로 승인한다.
2. CG-R01은 모듈·패키지 의존 그래프로 시작한다. 함수 호출 그래프는 이번 수용 기준에 넣지 않는다.
3. CG-R05는 프로젝트별 수동 설치로 시작한다. installer 포함은 효과 관측 후 별도로 정한다.
4. 1계층(CG-R01~R04)을 먼저 전달하고 2계층(CG-R05~R06)을 이어서 진행한다.

CG-R03의 간접 영향 추적 깊이 기본값은 요구 의미가 아니라 설계 선택으로 보고 Phase 1에서 정한다. 결과에는 적용한 깊이를 함께 기록한다.
