# 코드 그래프 도구 작업 계약

2026-10-28. 상태: 1계층 구현과 Phase 2 검증 진행. 목적 가설과 CG-R01~R06의 의미는 같은 날 BOSS가 “권장안대로 확정”으로 확인했다. 외부 도구 설치와 시험 프로젝트 적용은 각각 별도 확인을 받는다.

## 요청과 결정 경과

- BOSS 원래 표현: “태극의 코드 그래프 도구를 도입하고 싶어”, 이후 “2계층안으로 진행”.
- 같은 날 BOSS가 고른 항목: 목적은 변경 영향 범위 파악·변경 부담 진단·에이전트 코드 탐색 세 가지, 방식은 기존 도구 연동, 대상은 시험 프로젝트 언어와 태극 자체.
- 2계층안: 증거용 결정적 그래프(1계층)와 에이전트 탐색용 LSP 도구(2계층)를 분리한다.

## 목적 가설

변경 지점에서 의존·호출 관계를 실제 도구 출력으로 확인할 수 있으면 재검증 대상 선택과 변경 부담 진단이 추정이 아니라 관측에 기반하고, 에이전트가 시험 프로젝트 구조를 파악하는 반복 탐색이 줄어든다. 도구 설치나 그래프 생성 자체를 목적 달성으로 보지 않는다. 효과는 Phase 4에서 실제 작업 사례로 관측한다.

## 대상

| 저장소 | 언어 | 확인 근거 |
| --- | --- | --- |
| hoplites | Python | `git ls-files` 확장자 집계 (2026-10-28). `__path__`를 읽거나 바꾸는 코드는 패키지 탐색 경로를 바꿀 수 있으므로 `sys_path_unresolved` gap이다. `sys.path` 항목은 OS가 걷는 대로 ROOT 안 링크를 따라 실제 디렉터리로 바꾸고, 따라간 링크는 `read`에 넣어 대상 문자열까지 지문에 기록한다. 그렇게 추가한 디렉터리에 프로젝트 `os`·`pathlib`가 있으면 그 뒤 경로 값을 믿지 않고 gap이다. `__init__.py` 없는 하위 디렉터리(namespace package)를 import하면 그 위 패키지의 `__init__.py`로 edge를 둔다(최상위 디렉터리는 설치된 같은 이름 모듈에 질 수 있고 의존할 파일도 없으므로 제외). `sys`·`builtins` 허용 목록은 공개 속성만 통과시키고 `__dict__` 등 dunder 속성, `os.sys`처럼 다른 모듈 속성으로 닿는 경우, `__builtins__`는 gap이다 |
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
| Elixir | 기존 빌드의 compile manifest(`_build/<env>/lib/<app>/.mix/compile.elixir`)를 CLI에 내장한 고정 스크립트로 `elixir -e`(ROOT 밖 임시 디렉터리)에서 읽는다. manifest의 source별 compile·export·runtime 참조가 `mix xref graph`가 보고하는 관계다. `mix`는 실행하지 않는다(`mix.exs` 평가는 대상 프로젝트 코드 실행이므로) | 대상의 `_build`·`deps`를 만들거나 바꾸지 않고 compile하지 않는다. manifest 버전 35(Elixir 1.20)만 해석하고 다른 버전은 FAIL. 앱 이름은 `mix.exs`를 실행하지 않고 텍스트에서 읽는다. manifest는 ROOT 안 일반 파일·256 MiB 이하여야 하고, 읽는 부분(모듈·source 색인, 참조 목록)은 열거 전에 함수·프로세스 핸들·struct 없는 정해진 모양의 데이터인지 검사한다(struct 열거는 코드 실행이므로). `_build`가 없거나, manifest에 기록된 source의 내용 digest(Mix와 같은 blake2b)가 지금 파일과 다르거나, manifest가 컴파일하는 최상위 디렉터리(`lib` 등)에 manifest에 없는 tracked `.ex`가 있거나, manifest에 더 이상 tracked가 아닌 파일이 있으면(compile 후 삭제) FAIL. 동적 호출은 Elixir 자체 파서(`Code.string_to_quoted`, 구문 해석만 하고 실행하지 않음)로 찾는다. 원격 호출 `recv.fun`은 수신자가 별칭·atom·`__MODULE__`일 때만 정적이고(`alias Kernel, as: K`와 그 연쇄(8단계를 넘어 계속되면 모호), atom 별칭 `alias :"Elixir.Kernel", as: K`, `Elixir.Kernel`, `:"Elixir.Kernel"`은 같은 이름으로 정규화해 판정하고(`Elixir.`로 시작하는 절대 이름에는 지역 별칭을 적용하지 않음), 읽을 수 없는 별칭 대상의 짧은 이름은 모호로 보며, 한 파일에서 같은 짧은 이름이 여러 모듈에 묶이면 위치별 scope를 추적하지 않으므로 그 이름을 통한 호출은 gap, `import`로 들어온 `apply`·`concat`·`to_atom` 등 지역 호출도 gap), 그 밖의 수신자(변수, 모듈 attribute, 괄호 식, 같은 모양의 맵 필드 접근; 공백·줄바꿈 무관)와 `apply`·`Kernel.apply`·`:erlang.apply`·`Module.concat`·`Module.safe_concat`·`String.to_atom`·`String.to_existing_atom`(capture `&apply/3` 포함), `Function.capture`(import한 `capture/3` 포함), 모든 익명 함수 호출 `f.(...)`, 런타임 모듈·atom 함수 이름·인자 목록이 이어지는 인자나 3-tuple(`spawn(m, :run, [])`, `{m, :start_link, []}`)은 `dynamic_call` gap. 함수 이름까지 런타임 값이면(`spawn(m, f, a)`) 모양만으로는 일반 데이터와 같으므로 3-tuple과 MFA를 호출하는 함수(`spawn*`, `apply*`, `hibernate`, `async*`, `start*`)의 인자에서만 gap으로 본다. 여기서 모듈 자리는 별칭·atom·`__MODULE__`만 정적이고 그 밖의 식(변수, `List.first(mods)`, `state.release` 등)은 런타임 값으로 본다. 문자열·주석은 코드가 아니므로 대상이 아니다. 파싱되지 않는 파일은 파일 전체가 gap |
| Go | `go list -e -deps -test -json ./...` | main module 패키지만 노드(test 변형과, build cache의 생성 소스를 가진 test main 제외; `.test`로 끝나는 실제 패키지는 유지). test 전용 import를 포함해 패키지 `Error`·`DepsErrors`가 있으면 FAIL. `GOPROXY=off`와 함께 `GOPRIVATE`·`GONOPROXY`·`GONOSUMDB`를 비우고 `GOSUMDB=off`·`GOVCS=*:off`·`GOTOOLCHAIN=local`로 모듈·toolchain 내려받기를 막는다. 실행 후 go가 보고한 패키지 파일이 ROOT 안에 있는지 검사한다(실행 전 검사는 아래 ROOT 검사). `//go:embed`를 쓰는 패키지는 디렉터리 트리 전체를 입력 지문에 넣는다(임베드 파일은 종류·ignore와 무관하게 읽히고, 추가·삭제로 매칭이 바뀌므로). 이 트리는 실제 경로가 ROOT 안일 때만, 빌드 출력 표시(`_build`·`target`)로 건너뛴 곳도 따로 순회한다(`//go:embed _build/x`처럼 임베드가 그 안을 가리킬 수 있으므로). go가 부모 디렉터리에서 찾는 `go.work`는 ROOT 안의 것만 쓰고(없으면 `GOWORK=off`), main module의 `go.mod`도 소스처럼 ROOT 안 일반 파일이어야 한다. `plugin`을 import하는 패키지는 런타임에 코드를 적재하므로 `dynamic_load` gap이다. 오류 메시지에 넣는 패키지 이름·경로는 go가 보고한 값이 ROOT 안 경로인 경우만 쓰고 나머지는 개수로 줄인다. build tag·`go:generate` 산출물은 현재 환경 기준 |
| Rust | `cargo metadata --no-deps --offline`으로 workspace crate·target·path 의존을 얻고, `syn`으로 각 target의 `mod` 선언과 `use crate::`·`super::`·`self::`·workspace crate 경로를 파일 노드로 해석 | `cfg` 분기는 모두 포함, raw identifier(`r#type`)는 일반 이름과 같게 보고, 모듈 파일을 찾을 때 따라간 ROOT 안 링크는 `read`에 넣어 대상 문자열까지 지문에 기록, `#[path]` 지원(모듈 파일은 OS가 여는 대로 디렉터리의 링크를 먼저 따라간 뒤 `..`를 해석하고 파일 이름은 유지), cfg로 막힌 없는 `mod` 파일은 gap. 매크로 호출 안의 경로와 매크로가 만드는 `mod`·`use`는 분석하지 않는다(결과 limits). glob import는 대상 모듈 파일로의 edge가 된다. 외부 crate 이름은 의존성 rename 또는 library target 이름을 쓴다. 확장 상한(경로 16개·별칭 깊이 8·별칭 확장 1,024단계)이나 해석 예산(언급 하나당 4,096 상태, 방문 상태 중복 제거)에 걸려 버린 해석은 `resolution_truncated` gap. 매크로 인자는 쉼표로 구분된 식으로 해석되면 일반 코드처럼 해석한다. 허용 목록: 인자 외에 의존을 만들지 않는 표준 매크로(`println`·`vec`·`assert` 등 31개) 중 인자가 해석되는 호출(쉼표 구분 식, `vec![x; n]`, `matches!(x, 패턴)`), 내장 attribute(`allow`·`cfg`·`test`·`repr`·`default` 등 42개와 `clippy`·`rustfmt`·`diagnostic` 도구 attribute), 표준 trait derive(`Debug`·`Clone` 등 9개). 그 밖의 매크로·attribute·derive(`include!`, `json!`, `#[serde]`, `#[derive(Serialize)]`, attribute 매크로, `use`에 붙은 것 포함), 블록 안의 `mod`, 파일 내부 attribute, syn이 해석하지 못한 구문(`Verbatim`)은 `construct_not_analyzed` gap. crate 안에 같은 이름의 `macro_rules!`가 있거나 `#[macro_use] extern crate`가 있거나 `use`로 같은 이름이 묶이면 표준 매크로로 보지 않는다. `extern crate x as y`는 `use x as y`처럼 해석한다. `::name` 절대 경로는 별칭이 아니라 외부 crate로 해석한다. edition 2015 crate(edition 생략 포함)에서는 `::name`과 `use` 경로를 crate 루트 기준으로 해석한다. 표준 trait 이름이 `use`로 다른 derive에 묶이거나(`use x::Custom as Clone;`) crate·표준 라이브러리 밖의 glob import가 scope에 있으면 그 이름의 derive(`cfg_attr` 안 포함)와 표준 이름 매크로는 gap. `std`·`core`·`alloc` 자체가 `use`로 다시 묶이면 그 접두사의 매크로·derive도 gap. 중첩 `cfg_attr`의 `path`도 `cfg_attr_path` gap. 모듈 중첩이 64를 넘으면 FAIL. `cfg_attr(..., path = ...)`가 붙은 `mod`는 기본 파일만 분석하고 `cfg_attr_path` gap. 재노출을 따라 다른 crate로 들어가면 그 crate의 의존성 이름으로 해석한다. `use`가 묶은 이름(`use crate::a as b`, `use crate::a`)은 그 이름을 선언한 모듈·블록 scope 기준으로 펼친 경로를 그 scope 안에서 쓰고(cfg 분기마다 다른 바인딩은 모두 보존), `b::sub::f` 같은 경로를 해석한다. glob import 뒤 첫 경로 조각이 다른 곳에서 해석되지 않으면 glob 대상의 하위 모듈로 해석한다. 경로가 모듈의 `use` 바인딩(`pub use crate::imp as facade` 같은 재노출)이나 glob 재노출을 지나면 그 대상까지 따라간다. cfg 분기가 같은 이름의 `mod`를 여러 번 선언하면 분기마다 노드와 edge를 따로 둔다. `#[path]`가 현재 `mod` 사슬의 파일로 되돌아오거나 한 target의 모듈 파일 방문이 10,000을 넘으면 FAIL. `RUSTUP_AUTO_INSTALL=0`으로 toolchain 자동 설치를 막는다. CLI에 `syn` 의존을 추가한다 |
| Python | CLI에 내장한 표준 라이브러리 전용 스크립트를 `python3 -I`로 실행해 `ast`로 import 수집 | tracked `.py`만. 패키지 root는 `pyproject.toml`의 설정, `src/` 배치, 저장소 최상위 순으로 정한다. 같은 이름의 패키지와 모듈 파일이 있으면 절대·상대 import 모두 Python처럼 패키지를 택한다. 패키지에서의 wildcard import(`from pkg import *`)는 `__all__`의 하위 모듈을 불러올 수 있으므로 `dynamic_import` gap. 상대 import는 패키지 기준으로 해석하고 기준 패키지부터 중간 패키지까지의 `__init__.py`를 edge로 넣으며, 여러 파일에 대응되는 이름은 edge를 만들지 않고 gap으로 기록. `importlib`·`__import__`는 gap. `sys.path` 대입·slice 대입·다른 이름으로 묶기·`from sys import path`는 `sys_path_unresolved` gap. `import sys as s`, `from importlib import import_module as load` 같은 별칭도 추적하고, 별칭 사슬과 무관하게 `importlib`·`runpy`·`pkgutil`·`imp`·`zipimport` import, 이름·속성·문자열로 쓴 동적 import 기구(`builtins.__import__`, `"__import__"` 등), 코드 실행 builtin(`exec`·`eval`·`compile`), `sys.modules`가 있는 줄은 모두 `dynamic_import` gap. `sys`·`builtins`는 허용 목록으로 다룬다: 최상위 문장 자체인 `sys.path.insert/append` 호출과 `x in sys.path` 읽기만 평가·허용하고(경로 함수는 이 파일에서 `pathlib`·`os.path`·`os`의 원래 함수에 절대 import로 묶이고 다른 곳에서 다시 묶이지 않은 이름만 인정하며, 이 파일의 import 검색 경로에서 같은 이름의 프로젝트 모듈(`scripts/pathlib.py` 등)이 보이면 그 모듈의 이름은 인정하지 않음, 한 이름이 서로 다른 경로 함수에 묶이면(`dirname as locate`와 `abspath as locate`) 위치별로 추적하지 않으므로 인정하지 않음, wildcard import가 있는 파일은 어떤 이름도 다시 묶일 수 있으므로 `sys.path` 변경을 평가하지 않고 gap, `__file__`을 다시 묶는 파일은 `__file__` 기반 경로를 평가하지 않고 gap, 구조 분해·`for`·`with`·walrus(대입 우변 안 포함)·match 패턴·import 등으로 다시 묶인 경로 변수는 값을 모르는 것으로 보고, 경로 값은 pathlib·`os.path` 문자열처럼 `..`를 그대로 두고(`(p / "sub" / "..").parent`는 `p/sub`) `abspath`만 문자열을 정규화하며, `os.path.join`의 `.`·빈 조각은 `dirname` 결과가 달라지므로 gap이다. `..`는 경로가 쓰이는 곳(`sys.path` 항목, `resolve()`·`realpath`)에서 OS처럼 앞의 링크를 따라간 뒤 해석한다, `pathlib.Path = ...`·`setattr(os.path, ...)`처럼 모듈 속성을 바꾸는 파일은 경로 함수를 인정하지 않음, `resolve()`·`realpath`는 ROOT 안 링크를 따라감)(불리언·조건식·comprehension 안의 호출은 gap), `sys.meta_path`·`path_hooks`·`path_importer_cache`와 `from sys import path/modules/...`, `from builtins import exec/...`도 gap이며, 그 밖의 `sys.path`·`sys` 사용(별칭, 타입 주석 대입, walrus, 인자 전달, 첨자)은 `sys_path_unresolved` gap, `builtins`(별칭 포함)의 코드 실행·import 속성 사용이나 값으로 쓰기는 `dynamic_import` gap. module 최상위에서 항상 실행되는 문장(식·대입)의 `sys.path` 변경만 문장 순서대로 그 시점의 변수 값으로 평가하고, 함수·lambda·클래스 안이나 `if`·`try`·`for`·`with` 같은 조건부 블록 안의 변경은 모두 gap. `parents[N]`은 경로 깊이를 먼저 확인한다 |

build는 분석기 실행 전에 ROOT를 파일 시스템에서 직접 순회한다(git은 특수 파일을 목록에 넣지 않으므로). go·cargo·mix가 우리가 나열하지 않는 파일(untracked·ignored `.go`, workspace 멤버 `Cargo.toml`, `replace`된 로컬 모듈 등)도 읽기 때문이다. tracked·untracked 경로는 일반 파일·디렉터리이거나 ROOT 안의 그런 대상으로 가는 링크여야 한다. ignored 경로는 `.venv`처럼 ROOT 밖으로 가는 정상 링크를 담으므로 특수 파일(FIFO·장치·소켓), 그것을 가리키는 링크, go가 순회하는 위치에서 ROOT 밖 디렉터리를 가리키는 링크만 거부한다. cargo가 읽은 manifest는 실행 후 ROOT 안인지 검사한다. 위반이 있으면 모든 분석기를 FAIL로 둔다. `.git`과 도구가 만든 ignore된 빌드 출력만 건너뛴다. 이름이 아니라 도구가 남긴 표시로 판단한다: Mix 빌드(`_build/<env>/lib`, go는 `_`로 시작하는 디렉터리를 읽지 않음)와, go가 선택되지 않았을 때 `.rustc_info.json`·`CACHEDIR.TAG`가 있는 cargo·캐시 디렉터리. 이름만 `target`인 디렉터리는 cargo workspace 멤버가 있을 수 있어 검사한다. 이 표시는 로컬에서 생성되며 저장소 내용으로 들어오지 않는다. Elixir `_build`는 복사할 때 시작 디렉터리가 ROOT 안인지, 특수 파일·ROOT 밖 링크가 없는지 검사한다. `--out`은 링크를 따라 쓰지 않는다: 기존 링크면 거부하고, ROOT 안 경로가 링크된 디렉터리를 거쳐 ROOT 밖으로 나가면 거부한다. 쓰기는 출력 디렉터리를 `/`부터 링크를 따르지 않고(`O_NOFOLLOW`) 한 단계씩 연 디렉터리 핸들 기준으로 새 임시 파일을 만들고(`openat`) `renameat`으로 교체한다. 검사 뒤 대상이나 상위 디렉터리가 링크·FIFO로 바뀌어도 따라가거나 열지 않는다. 이를 위해 이미 lock에 있던 `libc` 0.2.190을 직접 의존으로 선언했다. 그래프에는 선택된 분석기가 읽는 입력 파일 전체(링크는 대상 문자열도, ROOT 밖으로 가는 대상은 내용 대신 표시만)(언어별 소스 패턴, ignored 포함, Python 소스는 tracked·untracked만)와 분석기가 실제로 읽었다고 보고한 파일(`read`: Mix manifest의 접근 경로와 실제 파일(링크가 다른 manifest로 바뀌어도 감지), `pyproject.toml`(없을 때도 기록해 생기면 바뀜), 다른 확장자의 `#[path]` 파일, cargo manifest)의 해시 `inputs`를 기록한다. 각 분석기는 자식 프로세스로 격리해 실행하고(자식의 `TMPDIR`은 부모가 만들고 지우는 0700 디렉터리이며, 모든 임시 디렉터리는 0700으로 만든다), 시간 제한(기본 600초, `TAEGUK_GRAPH_TIMEOUT_SECS`)을 둔다. 이 시간 제한은 분석기만이 아니라 CLI가 시작하는 모든 프로세스(git, 버전 조회, 도구)에 같이 걸린다(git도 FIFO `.gitignore`를 열면 멈추므로). CLI 프로세스는 자식마다 새 프로세스 그룹을 만들고 자식이 끝나거나 시간 초과로 멈추면 남은 그룹을 정리한다. 분석기 안에서 실행한 도구는 분석기 그룹에 남아 함께 정리된다. 분석기 입력(언어별 소스 패턴과 `read`) 중 하나라도 실제 경로가 ROOT 밖이면 도구가 그것을 읽었을 수 있으므로 build는 모든 분석기를 FAIL로 두고 결과를 버린다. impact는 그래프의 `read`가 절대 경로·`.`·`..`·빈 구성 요소 없는 ROOT 상대 경로(트리는 `dir/`, ROOT 전체 트리는 `./`)인지 먼저 확인하고, build도 기록하기 전에 같은 검사를 해 impact가 거부할 그래프를 쓰지 않는다. 비정상 종료나 시간 초과는 그 분석기의 FAIL이다. 모든 분석기는 source(`mix.exs`·`pyproject.toml` 포함)를 읽기 전에 열지 않고 실제 경로가 ROOT 안의 일반 파일이고 8 MiB 이하인지 확인하고(FIFO는 열면 멈추므로), 아니면 FAIL한다. 버전 기록용 `--version` 명령도 ROOT에서 같은 toolchain 차단 환경변수로 실행한다.

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

### 정적 분석과 LLM 검토의 분담

태극 보완 원칙(정적으로 덮고, 정적으로 덮지 못한 영역은 LLM으로 덮는다)에 따라 나눈다. 2026-10-29 BOSS가 이 방식으로 진행을 확인했다.

- 정적 층(`taeguk graph`): 해석 가능한 관계는 edge로 낸다. 해석하지 못한 지점은 빠짐없이 gap으로 낸다. gap 없이 조용히 누락하면 결함이다.
- LLM 층: impact가 영향 범위 밖 파일의 gap을 `llm_review`로 낸다. gap 파일은 정적으로 확인되지 않은 의존을 가질 수 있어 영향받는 파일에 닿을 수 있기 때문이다. LLM 검토가 각 지점을 읽고 영향받는 파일에 닿는지 근거와 함께 기록하기 전에는 영향 결과가 완결되지 않는다(`gaps_need_llm_review`로 UNKNOWN).
- gap 판정은 허용 목록 방식이다(2026-10-29 BOSS 확인). 분석기가 완전히 이해하는 구문만 목록으로 두고, 목록 밖 구문은 모두 gap으로 표시한다. 못 보는 구문을 하나씩 나열하는 방식은 리뷰마다 새 사례가 나와 수렴하지 않았다.
- 리뷰 지적 판정: 정적 정밀도 지적은 해당 사례가 gap으로 표시되는지로 판정한다. 표시되면 LLM 층의 몫이고, 표시되지 않으면 결함으로 고친다. 안전·보안 지적은 이 분담과 무관하게 고친다.

### 결함 판정 기준 (합의점)

1~9차 로컬 리뷰에서 같은 부류의 새 사례가 반복된 뒤 정한 기준이다. 이후 이 도구를 바꾸거나 리뷰하는 작업은 이 기준으로 지적을 판정한다. 기준에 맞는 결함은 고치고, 기준상 설계대로인 동작은 결함으로 다루지 않는다.

| 부류 | 결함 | 설계대로인 동작 |
| --- | --- | --- |
| 안전 | 대상 저장소 내용 때문에 CLI가 멈추거나 비정상 종료하거나, ROOT 밖 파일을 읽어 그래프에 넣거나, `--out` 밖에 쓴다. 분석기가 대상 프로젝트 코드(`mix.exs`, build script, 설정 스크립트 등)나 저장소가 지정한 프로그램(toolchain 경로 등)을 실행한다 | 분석기 하나가 자식 프로세스에서 비정상 종료하거나 시간 제한(기본 600초)에 걸려 그 언어가 FAIL이 된다 |
| 정확도 | 허용 목록 안의 구문을 잘못 해석한다. 허용 목록 밖의 구문이 gap 없이 지나간다 | 허용 목록 밖의 구문이 gap이 되어 `llm_review`로 넘어간다(오탐 포함) |
| 현재성 | 분석기 입력이 바뀌었는데 impact가 PASS다 | 입력 변경, 작업 트리 변경, 도구 버전 변경, gap 때문에 UNKNOWN이다 |

- 자원 부류는 사례별로 막지 않고 격리로 닫는다. 각 분석기는 `taeguk graph __analyze LANG ROOT` 자식 프로세스로 실행되고, 시간 제한에 걸리면 프로세스 그룹째 종료된다. 사례별 사전 검사(ROOT 순회, 크기 제한, 깊이 상한, 해석 예산)는 흔한 경우를 빨리 FAIL로 만드는 보조다.
- 분석기는 대상 프로젝트 코드를 실행하지 않는다. 언어 도구가 프로젝트 코드를 평가하는 명령(`mix` 태스크 등)은 쓰지 않고, 그 도구가 남긴 산출물을 고정 스크립트로 읽는다.
- 대상 저장소에서 온 문자열(파일 이름, ROOT 안 경로)은 도구의 명령줄 인자로 넘기지 않고 stdin으로 넘긴다(`-r` 같은 이름이 옵션으로 읽히지 않도록).
- git이 작업 트리 파일을 filter driver에 통과시키는 명령은 쓰지 않는다. attribute는 `core.attributesFile`·`.git/info/attributes`에서도 오므로 옵션으로 막을 수 없어서다. 작업 트리 변경은 index를 HEAD와 비교(`diff --cached`)하고, 작업 트리 파일은 이 CLI가 non-blocking으로 열어 열린 핸들로 일반 파일인지 확인한 뒤 git blob id(SHA-1, SHA-256 저장소는 SHA-256)를 직접 계산해 index와, 링크는 대상 문자열의 blob id를 계산해 비교한다(object store의 blob을 읽지 않음). FIFO 같은 특수 파일은 열지 않고 변경으로 본다. filter를 쓰는 저장소(LFS)는 늘 변경으로 보여 impact가 UNKNOWN이 된다. `--git-diff BASE`는 BASE..HEAD 트리 비교와 이 작업 트리 변경을 합친다. fsmonitor도 끈다(`core.fsmonitor=false`). 저장소 설정이 promisor remote를 두면 git은 없는 객체를 스스로 fetch하며 저장소가 고른 전송 명령(`core.sshCommand`)을 실행하므로, CLI가 시작하는 모든 프로세스에 `GIT_NO_LAZY_FETCH=1`과 빈 `GIT_ALLOW_PROTOCOL`(어떤 전송도 허용하지 않음)을 건다. 분석기와 그 안의 go·cargo도 이를 물려받는다. diff는 `--no-ext-diff --no-textconv`로 외부 diff·textconv를 쓰지 않고, go는 `-buildvcs=false`로 VCS를 부르지 않는다.
- 도구(go·cargo·python3·elixir)와 버전 조회는 모두 ROOT 밖 임시 디렉터리에서 실행한다(`go -C ROOT`, `cargo --manifest-path`). 도구 선택은 사용자 환경이 정하고, 대상 저장소의 `rust-toolchain.toml`(실행 파일 경로를 지정할 수 있음)·`.cargo/config.toml`·버전 관리자 설정은 쓰이지 않는다. ROOT 검사가 실패하면 어떤 도구도 실행하지 않는다.
- 부모 CLI가 읽는 파일(작업 트리 해시, 입력 지문, 그래프)은 non-blocking으로 열고 열린 핸들로 일반 파일인지 확인한다(검사 뒤 FIFO로 바뀌어도 멈추지 않음). 작업 트리 blob 해시는 64 KiB 단위로 스트리밍하고 1 GiB를 넘는 파일은 읽지 않고 변경으로 본다. 입력 지문은 크기 검사 뒤에도 실제 읽기를 8 MiB로 제한한다. 8 MiB를 넘는 입력은 읽지 않고 크기·inode·ctime·mtime으로 식별한다(mtime은 사용자가 되돌릴 수 있지만 ctime은 쓰기마다 커널이 갱신한다).
- 분석기가 직접 읽는 소스(Rust 모듈, `mix.exs`, Mix manifest, Elixir 소스, Python 소스)는 연 뒤에 커널이 보고하는 열린 파일의 경로(macOS `F_GETPATH`, Linux `/proc/self/fd`)가 ROOT 안인지 확인한다. 검사 뒤 파일이나 상위 디렉터리가 ROOT 밖 링크로 바뀌어도 그 내용은 그래프에 들어가지 않는다. `elixir`는 경로가 아니라 이렇게 읽은 내용을 임시 디렉터리에 복사한 파일을 읽는다. go·cargo가 실행 중 스스로 읽는 파일은 이 방식으로 감쌀 수 없어, 실행 전 ROOT 검사, 실행 후 보고된 파일의 ROOT 내부 검사, 분석 전후 ROOT 상태 비교로 다룬다. 상태 비교는 ROOT 순회가 방문하는 모든 항목(ROOT 자신 포함, 링크는 따라가지 않음)의 inode·ctime·mtime·크기·mode를 분석기 실행 전후로 비교해 하나라도 다르면 모든 분석기를 FAIL로 둔다. 파일을 ROOT 밖 링크로 바꿨다 되돌려도 ctime은 사용자가 되돌릴 수 없어 흔적이 남는다.
- 외부 도구(go·cargo)의 진단 원문은 그래프와 CLI 출력에 넣지 않는다. 진단은 읽은 파일을 인용하는데 그 파일이 링크로 ROOT 밖일 수 있어서다. 실패는 종료 코드와, 이름이 ROOT에서 온 패키지(ROOT 안 패키지나 그것이 import하는 패키지)만 남기고 원인은 도구를 직접 실행해 보라고 안내한다.
- impact가 읽는 그래프 파일도 입력이다: 열기 전에 일반 파일·256 MiB 이하인지 확인하고, digest는 서명이 아니므로 edge 끝점이 실제 노드인지, 노드 id가 중복되지 않는지 검증한다.
- 허용 목록을 넓히는 변경은 그 구문을 실제로 해석한다는 fixture와 함께만 한다.

### 영향 질의

- 역방향 edge 전이 폐쇄를 끝까지 따른다. 기본 깊이 제한은 두지 않고 각 노드에 최단 거리를 붙인다. `--depth N`으로 제한하면 적용 깊이를 결과에 기록한다. 순환은 방문 집합으로 종료한다.
- 그래프 `status`가 FAIL이거나, 그래프 commit이 현재 `ROOT`의 HEAD와 다르거나, 그래프 생성 때 dirty였거나, 질의 시점의 작업 트리·index가 HEAD와 다르거나 ignore되지 않은 untracked 파일이 있거나(분석기가 읽으므로, 생성 시 `dirty_paths`에도 기록), 분석기 입력 해시가 그래프의 `inputs`와 다르거나(ignored 입력 변경 포함), 그래프의 ROOT가 질의 ROOT와 다르거나, 기록된 분석 도구 버전이 현재 버전과 다르거나 확인되지 않거나, 변경 파일이 어느 노드에도 속하지 않거나, LLM 검토가 필요한 gap이 있으면 결과 `status`를 UNKNOWN으로 하고 이유를 남긴다. 영향 목록은 참고로 함께 낸다.
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

## 1계층 구현 중 설계 보완

- Python: hoplites 실측에서 `tests/*.py`가 `sys.path.insert(0, str(Path(__file__).parents[1] / "scripts"))` 뒤에 `scripts`를 import하는 관계 33건을 gap 없이 놓쳤다. CG-R01의 “해석되지 않는 관계는 누락 가능으로 표시” 위반이므로, module 수준 경로 변수와 `__file__`·`Path`·`parent(s)`·`/`·`os.path` 조합으로 된 `sys.path.insert/append`를 정적으로 평가해 검색 경로에 넣는다. 평가하지 못한 변경(`Path.cwd()` 등)은 `sys_path_unresolved` gap으로 남긴다. `spec_from_file_location`·`runpy.run_path`도 `dynamic_import` gap으로 기록한다.
- Rust: 부모 모듈의 `mod` 선언은 사용 관계가 아니므로 edge로 만들지 않는다. `use`, 식·타입 경로, `pub use` 재노출을 파일 edge로 만든다. 매크로 호출 안의 경로는 분석하지 않으며 이는 결과 `limits`와 이 문서의 한계로 남긴다.
- acropolis/akashic 본체는 bare 저장소라 작업 트리 `akashic-worktrees/personal-knowledge-reset`에서 실측했다.

## 검증 결과 (2026-10-29, 29차 리뷰 수정 반영본 기준)

| ID | 상태 | 근거 |
| --- | --- | --- |
| CG-V01 | PASS | `tools/taeguk-cli/tests/graph.rs`의 언어별 fixture 4개가 수작업 `expected-edges.txt`·`expected-gaps.txt`와 집합 일치. 기대 파일의 한 줄을 지웠을 때 네 언어 모두 테스트가 실패하는 것도 확인(오류 주입). 2차 수정으로 Python 기준 패키지 `__init__`(`src/app/sub/deep.py`)과 Rust `use` 별칭(`app/src/ops.rs`) 사례를 추가했고, 수정 전 코드에서는 두 fixture 테스트가 실패함을 확인. 3차 수정으로 Rust cfg 동명 모듈(`core/src/sys/`), scope별 별칭(`app/src/scoped.rs`), glob 하위 모듈(`app/src/globbed.rs`), Python `sys.path` 대입·거대 `parents` 인덱스(`tests/test_paths.py`)를 추가. 2차 반영본(`4114601`) 바이너리는 Rust 기대 edge 3개를 놓쳤고 `sys.path` 대입 gap을 내지 않았다. 4차 수정으로 Rust 재노출(`facade_user.rs`), cfg 분기 별칭(`plat.rs`), 선언 scope 별칭(`declared.rs`), Python 문장 순서(`tests/test_order.py`)를 추가. 3차 반영본(`2cdb901`) 바이너리는 Rust 기대 edge 4개, Python edge 1개·gap 1개를 놓쳤다. 5차 수정으로 crate를 넘는 재노출(`base` crate), 확장 상한 gap(`app/src/many.rs`), Python 별칭 gap(`tests/test_aliases.py`), Elixir 괄호 없는 `apply` gap(`lib/fx/dyn2.ex`)을 추가. 4차 반영본(`ed09248`) 바이너리는 edge 1개와 gap 4개를 놓쳤다. 6차 수정으로 매크로 인자(`app/src/macros.rs`), `cfg_attr` 경로, glob 재노출 순환(`app/src/loop_/`), 재대입한 동적 import(`tests/test_reload.py`), Elixir 변수 원격 호출(`lib/fx/runner.ex`)을 추가. 5차 반영본(`3f381de`) 바이너리는 edge 1개와 gap 4개를 놓쳤다. 7차 수정으로 `include!`(`app/src/inc.rs`), 속성 `__import__`(`tests/test_builtins.py`), 괄호 없는 변수 원격 호출(`lib/fx/runner2.ex`)을 추가. 6차 반영본(`6ac24b1`) 바이너리는 gap 3개를 놓쳤다. 8차 허용 목록 전환으로 허용 구문만 쓰는 `app/src/allow.rs`(gap 없음, `vec![x; n]`·`matches!` 인자 해석), 목록 밖 구문의 `app/src/hidden.rs`(블록 안 `mod`, attribute 매크로, 외부 derive, `vec![include!(..); 1]`), 함수 매개변수 경로(`tests/test_param.py`), 인자 없는 변수 점 접근(`lib/fx/runner3.ex`)을 추가. 7차 반영본(`a94f039`) 바이너리는 edge 1개와 gap 3개를 놓쳤다. 9차 수정으로 `extern crate` 별칭(`app/src/ext.rs`), `use`의 attribute 매크로(`app/src/used.rs`), 중첩 `cfg_attr` 경로(`app/src/main.rs`), `if` 안 함수의 `sys.path`(`tests/test_nested.py`), 문자열 `#` 뒤 호출(`lib/fx/runner4.ex`)을 추가. 8차 반영본(`0c7fde7`) 바이너리는 edge 2개와 gap 4개를 놓쳤다. 10차 수정으로 패키지 우선(`src/app/user.py`, `src/app/pkgmod.py`·`pkgmod/__init__.py`), 표준 이름 derive 재바인딩(`app/src/derived.rs`)을 추가. 9차 반영본(`eb3f559`) 바이너리는 edge 1개와 gap 1개를 놓쳤다. 11차 수정으로 `exec` import(`tests/test_exec.py`), 조건부 `sys.path`(`tests/test_cond.py`), 상대 import 패키지 우선(`src/app/rel.py`), `cfg_attr` derive 재바인딩(`app/src/cfgderive.rs`), 외부 glob의 매크로(`app/src/globmac.rs`)를 추가. 10차 반영본(`b1ce265`) 바이너리는 잘못된 edge 1개(`rel.py → pkgmod.py`)와 gap 4개를 놓쳤다. 12차 수정으로 타입 주석 `sys.path` 별칭(`tests/test_annotated.py`), `builtins` 별칭 `exec`(`tests/test_bexec.py`), 괄호 식 수신자(`lib/fx/runner5.ex`)를 추가. 11차 반영본(`53e3104`) 바이너리는 gap 3개를 놓쳤다. 13차 수정으로 `from builtins import exec`(`tests/test_fromexec.py`), 불리언 식 안 `sys.path`(`tests/test_boolop.py`), 줄바꿈 점 원격 호출과 문서 문자열 속 점(`lib/fx/runner6.ex`), `std` 재바인딩(`app/src/stdalias.rs`)을 추가. 12차 반영본(`ff36c62`) 바이너리는 gap 4개를 놓쳤다. 14차 수정으로 재정의된 `dirname`(`tests/test_shadow.py`), `alias Kernel, as: K`(`lib/fx/runner7.ex`)를 추가. 13차 반영본(`1f96706`) 바이너리는 gap 2개를 놓쳤고, 링크된 스크립트의 `Path(__file__).resolve()` 경로 import edge를 놓쳤다(새 바이너리는 `scripts/linked.py → lnk/sibling.py`). 15차 수정으로 패키지 wildcard import(`src/app/wilduser.py`), 상대 import한 동명 `pathlib`(`src/app/relpath.py`), `:"Elixir.Kernel".apply`(`lib/fx/runner8.ex`), `::fx_core` 절대 경로(`app/src/abs.rs`)를 추가. 14차 반영본(`1cefaf6`) 바이너리는 edge 1개와 gap 2개를 놓쳤다. 16차 수정으로 스크립트 옆의 사용자 `pathlib.py`(`plug/`), 같은 짧은 이름의 별칭 재바인딩(`lib/fx/runner9.ex`)을 추가. 15차 반영본(`689fe31`) 바이너리는 gap 2개를 놓쳤다. 17차 수정으로 edition 2015 crate(`legacy/`), wildcard import 뒤 `sys.path`(`tests/test_wild.py`), atom 별칭(`lib/fx/runner10.ex`)을 추가. 16차 반영본(`042cc79`) 바이너리는 edge 1개와 gap 2개를 놓쳤다. 18차 수정으로 구조 분해 재대입(`tests/test_unpack.py`), `__file__` 재대입(`tests/test_file.py`), 익명 호출과 `&apply/3`(`lib/fx/runner11.ex`)을 추가. 17차 반영본(`74cf935`) 바이너리는 gap 3개를 놓쳤다. 19차 수정으로 `.test`로 끝나는 실제 Go 패키지(`e.test/`), match 패턴 재바인딩(`tests/test_match.py`), 모듈 속성 교체(`tests/test_patch.py`), `spawn(m, :run, [])`(`lib/fx/runner12.ex`)을 추가. 18차 반영본(`b5c6656`) 바이너리는 edge 1개와 gap 3개를 놓쳤다. 20차 수정으로 대입 우변 walrus(`tests/test_walrus.py`), import한 `capture/3`(`lib/fx/runner13.ex`)을 추가. 19차 반영본(`071abc8`) 바이너리는 gap 2개를 놓쳤다. 21차 수정으로 import로 다시 묶인 경로 변수(`tests/test_importbind.py`), 식으로 계산된 MFA 모듈(`lib/fx/runner14.ex`)을 추가. 20차 반영본(`628e77d`) 바이너리는 gap 2개를 놓쳤다. 22차 수정으로 충돌하는 경로 함수 바인딩(`tests/test_rebind.py`), 10단계 별칭 연쇄(`lib/fx/runner15.ex`)를 추가. 21차 반영본(`aea2a27`) 바이너리는 gap 2개를 놓쳤다. 23차 수정으로 `__path__` 확장(`src/app/nspkg/__init__.py`), `plugin` import(`go/f`)를 추가. 22차 반영본(`75e82f6`) 바이너리는 gap 2개를 놓쳤다. 24차 수정으로 `sys.path`에 추가한 디렉터리의 `pathlib`(`plugrun/run.py`, edge `plugrun/run.py → plug/pathlib.py` 포함), 함수 이름까지 런타임 값인 `spawn(m, f, a)`(`lib/fx/runner16.ex`)와 MFA를 받지 않는 `GenServer.call(server, request, timeout)`(`lib/fx/plain.ex`, gap 없음)을 추가. 23차 반영본(`61b9916`) 바이너리는 gap 2개를 놓쳤고, ignore된 링크를 거치는 `sys.path` 항목의 import edge(`tr/run.py → ta/helper.py`)도 gap 없이 놓쳤다. 25차 수정으로 `..` 뒤 `.parent`·`dirname`(`updown/run.py`, `updown/run2.py`, 각각 edge `→ updown/sub/helper.py`)을 추가. 24차 반영본(`7ea3928`) 바이너리는 두 edge를 gap 없이 놓쳤다. 26차 수정으로 `__init__.py` 없는 하위 디렉터리 import(`src/app/nsuser.py`의 `import app.nsdir`, edge `→ src/app/__init__.py`)를 추가. 25차 반영본(`9c680e2`) 바이너리는 이 edge를 gap 없이 놓쳤다. 27차 수정으로 raw identifier 모듈과 경로(`mod r#type;`, `crate::r#type::g()`의 `app/src/rawuser.rs → app/src/type.rs`)를 추가. 26차 반영본(`3422a68`) 바이너리는 `r#type.rs`를 찾다가 Rust fixture 전체가 FAIL이었다. 28차 수정으로 `builtins.__dict__["exec"]`·`sys.__dict__["path"]`(`tests/test_dict.py`), `os.sys.path`(`tests/test_ossys.py`)를 추가. 27차 반영본(`520dd0b`) 바이너리는 gap 3개를 놓쳤다. 29차 수정으로 `from builtins import __dict__`(`tests/test_fromdict.py`), `from sys import __dict__`(`tests/test_fromsysdict.py`)를 추가. 28차 반영본(`ba40003`) 바이너리는 gap 3개를 놓쳤다. tracked 링크는 대상이 같으면 dirty가 아니고, 대상을 바꾸면 그 경로만 dirty다. Elixir 현재성은 mtime 대신 Mix 내용 digest로 판정한다(컴파일 직후 내용을 바꾸면 `changed since the last compile`). Elixir fixture는 manifest reader로 바꾼 뒤에도 수작업 기대 edge와 일치 |
| CG-V02 | PASS | 구문 오류 Python, 없는 모듈을 import하는 Go, mod 파일이 없는 Rust, `_build` 없는 Elixir에서 `status: FAIL`, 원인 문자열, 노드 0개. 2차 수정 사례: `/dev/zero` 링크 `.py`, test 전용 missing import Go, `#[path]` 순환 Rust, compile 후 삭제한 Elixir 소스가 FAIL. 수정 전 바이너리에서 각각 메모리 약 14 GB까지 읽기 지속, PASS, 다른 오류, PASS였다. compile과 같은 초에 저장된 소스는 PASS(밀리초 비교로 되돌리면 실패). 3차 수정 사례: `/dev/zero` 링크 `mix.exs`, FIFO로 바꾼 tracked `.py`, ROOT 밖 파일 링크 `.go`, 단계마다 형제 `mod` 2개가 다음 파일을 가리키는 30단계 Rust가 FAIL. `4114601` 바이너리는 FIFO와 30단계 Rust에서 15초 안에 끝나지 않았고 ROOT 밖 `.go`는 PASS였다. 4차 수정 사례: Go 디렉터리의 untracked FIFO, `testdata/evil/go.mod`의 `/dev/zero` 링크, workspace 멤버 `Cargo.toml`의 `/dev/zero` 링크, Elixir `_build`의 FIFO가 FAIL. git 목록 기반이던 첫 구현은 untracked FIFO에서 `go list`가 멈춰 직접 순회로 바꿨다. `2cdb901` 바이너리는 `_build` FIFO에서 15초 안에 끝나지 않았다. 5차 수정 사례: `--out`이 ROOT 밖 파일로 가는 링크·링크된 디렉터리면 거부하고 대상 파일 불변, ignore된 디렉터리의 Go FIFO, ROOT 밖을 가리키는 `_build/dev`가 FAIL. ignore된 `.venv`의 ROOT 밖 링크는 PASS. `ed09248` 바이너리는 링크를 따라 외부 파일을 덮어썼고, `_build/dev` 링크는 PASS, ignore된 FIFO에서는 15초 안에 끝나지 않았다. 6차 수정 사례: go 선택 시 ignore된 `target`의 FIFO는 FAIL, 기존 FIFO인 `--out`은 일반 파일로 교체, 17개 모듈 glob 재노출 순환은 0초에 PASS. `3f381de` 바이너리는 세 사례 모두 20초 안에 끝나지 않았다. 7차 수정 사례: ignore된 `target/member`의 FIFO `Cargo.toml`이 FAIL, cfg 분기 별칭 32개가 서로 묶인 파일은 0초에 PASS와 `resolution_truncated` gap. `6ac24b1` 바이너리는 두 사례 모두 20초 안에 끝나지 않았다. 8차 수정 사례: 80단계 `#[path]` 사슬은 깊이 64에서 FAIL(`a94f039` 바이너리는 3,000단계 사슬에서 stack overflow로 비정상 종료), ignore된 `target/member` 소스 변경은 `analyzer_inputs_changed`. 9차 수정 사례: 20만 겹 중첩 `mod`는 분석기 자식 프로세스의 stack overflow를 FAIL로 기록(`0c7fde7` 바이너리는 CLI 자체가 비정상 종료하고 그래프를 쓰지 못함), ROOT 밖 FIFO manifest를 cargo가 읽는 사례는 시간 제한 3초에서 FAIL, go가 순회하는 ignore된 ROOT 밖 디렉터리 링크는 FAIL, crate 안에서 재정의된 `vec!`은 gap. 10차 수정 사례: `mix.exs` 최상위에 파일 쓰기를 넣은 저장소에서 그래프 생성 후 표지 파일 없음(`eb3f559` 바이너리는 실행해 표지 파일 생성), ignore된 `pyproject.toml`과 ignore된 `#[path = "gen.inc"]` 파일 변경은 `analyzer_inputs_changed`, compile manifest가 FIFO거나 `_build/dev`가 ROOT 밖 링크면 FAIL. 11차 수정 사례: `rust-toolchain.toml`의 `path`가 저장소 안 가짜 `cargo`를 가리켜도 실행되지 않고 PASS(`b1ce265` 바이너리는 실행해 표지 파일 생성), `rust-toolchain.toml`이 FIFO면 도구를 실행하지 않고 FAIL과 버전 미확인, 없던 `pyproject.toml`이 생기면 `analyzer_inputs_changed`. 13차 수정 사례: checkout의 `core.fsmonitor`·`filter.*.clean` 설정이 build·impact 중 실행되지 않음(`ff36c62` 바이너리는 fsmonitor 명령 실행), callback을 가진 `Stream`을 넣은 manifest는 열거 전에 거부되고 callback 미실행(`ff36c62` 바이너리는 실행). 14차 수정 사례: manifest에 `-r`이라는 source를 넣고 같은 이름의 tracked 파일에 코드를 둬도 실행되지 않음(`1f96706` 바이너리는 `elixir -r`로 실행). 15차 수정 사례: `core.attributesFile`·`.git/info/attributes`로 지정한 clean filter가 build·impact 중 실행되지 않음(`1cefaf6` 바이너리는 실행). 16차 수정 사례: compile manifest 링크를 다른 manifest로 바꾸면 `analyzer_inputs_changed`(`689fe31` 바이너리는 이유 없이 통과). 18차 수정 사례: FIFO를 그래프로 주면 열지 않고 오류(`74cf935` 바이너리는 10초 안에 끝나지 않음), digest를 다시 계산한 그래프에 없는 노드로 가는 edge를 넣으면 panic 대신 오류(exit 2). 20차 수정 사례: tracked 파일을 2 GiB sparse 파일로 키우면 읽지 않고 변경으로 보고 build가 최대 RSS 32 MB·0.17초(`071abc8` 바이너리는 최대 RSS 6.4 GB·102초), 링크 뒤 `..`는 gap. 22차 수정 사례: ignore된 workspace 멤버 `Cargo.toml`이 ROOT 밖 파일로 가는 링크일 때 그 내용이 그래프에 남지 않음(`aea2a27` 바이너리는 cargo 진단을 통해 1회 노출), ignore된 임베드 파일을 지우면 `analyzer_inputs_changed`(`aea2a27` 바이너리는 이유 없이 통과). 23차 수정 사례: `//go:embed _build/x`의 임베드 파일 변경은 `analyzer_inputs_changed`(`75e82f6` 바이너리는 이유 없이 통과), 실패한 go 패키지의 ROOT 밖 이름이 오류에 남지 않음(`75e82f6` 바이너리는 1회 노출), 분석기 임시 디렉터리는 0700이고 실행 후 남지 않음. 24차 수정 사례: `sys.path`가 거치는 ignore된 링크를 다른 디렉터리로 바꾸면 `analyzer_inputs_changed`, 그래프의 `read`에 `../`·절대 경로·빈 경로를 넣으면 digest를 다시 계산해도 순회 전에 오류(`61b9916` 바이너리는 받아들임). 25차 수정 사례: 모듈 최상위 Go 패키지의 `//go:embed`는 `read`에 `./`로 기록되고 impact가 받아들이며 임베드 파일 변경은 `analyzer_inputs_changed`(`7ea3928` 바이너리의 그래프는 impact가 거부), `.gitignore`를 FIFO로 바꾸면 build·impact가 시간 제한 2초에 오류로 끝남(`7ea3928` 바이너리는 10분 넘게 멈춤), 분석기를 시간 초과로 멈춘 뒤 그 분석기가 시작한 cargo가 남지 않음, 링크 뒤 `..`는 gap 대신 OS처럼 링크 대상에서 올라가 edge(`dotdot.py → sub/sibling.py`). 26차 수정 사례: Rust `#[path = "lnk/../choice.rs"]`(`lnk`는 `real/deep` 링크)는 `app/src/real/choice.rs`로 해석(`9c680e2` 바이너리는 `app/src/choice.rs`), ignore된 `go.mod`가 ROOT 밖 manifest 링크면 FAIL이고 그 module 경로가 그래프에 남지 않음(`9c680e2` 바이너리는 노드에 기록), ROOT 위 디렉터리의 잘못된 `go.work`는 쓰이지 않고 PASS. 27차 수정 사례: ROOT 안 `go.work`가 `use ../private`로 ROOT 밖 module을 넣으면 FAIL이고, 그 module 소스만 이름을 아는 패키지(`internal/secretpkg`)는 오류에 나오지 않음(`3422a68` 바이너리는 오류에 노출). 28차 수정 사례: `#[path = "route/m.rs"]`가 거치는 ignore된 디렉터리 링크를 같은 내용의 다른 디렉터리로 바꾸면 `analyzer_inputs_changed`(`520dd0b` 바이너리는 이유 없이 통과), 분석 중 `go.mod`를 ROOT 밖 링크로 바꿨다 되돌리면 ROOT 상태 비교가 감지(단위 테스트), akashic build 시간은 57초→58초. 29차 수정 사례: promisor remote와 저장소가 지정한 `core.sshCommand`를 둔 저장소에서 HEAD tree를 지워도 build·impact가 그 명령을 실행하지 않음(`ba40003` 바이너리는 실행해 표지 파일 생성), ignore된 9 MiB 임베드 파일을 같은 크기로 바꾸고 mtime을 되돌려도 `analyzer_inputs_changed`(`ba40003` 바이너리는 이유 없이 통과) |
| CG-V03 | PASS | 29차 수정본 재실측: 6곳 PASS, edge·gap 집합이 28차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 28차 수정본 재실측: 6곳 PASS(분석 전후 ROOT 상태 비교로 생긴 FAIL 없음), edge·gap·`read` 집합이 27차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 27차 수정본 재실측: 6곳 PASS, edge·gap 집합이 26차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 26차 수정본 재실측: 6곳 PASS, edge·gap 집합이 25차 수정본과 동일(입력이 ROOT 밖이면 FAIL로 바꾼 뒤에도 `.venv` 링크 등으로 FAIL이 생기지 않음), 대상 `git status` 불변. 이전 기록: 25차 수정본 재실측: 6곳 PASS, edge·gap·`read` 집합이 24차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 24차 수정본 재실측: 6곳 PASS, edge·gap·`read` 집합이 23차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 23차 수정본 재실측: 6곳 PASS, edge·gap 집합이 22차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 22차 수정본 재실측: 6곳 PASS, edge·gap 집합이 21차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 21차 수정본 재실측: 6곳 PASS, edge 집합 동일, axiom `dynamic_call` 19→20(늘어난 1줄 `Keyword.get(opts, :release_id, state.release)`는 런타임 값 모양이라 보수적으로 잡힌 오탐), thesmothetai는 대상 저장소에 새 commit이 생겨 gap 3→0, 대상 `git status` 불변. 이전 기록: 20차 수정본 재실측: 6곳 PASS, edge·gap 집합 동일(열린 파일 경로 확인을 거쳐도 결과 불변), 대상 `git status` 불변. 이전 기록: 19차 수정본 재실측: 6곳 PASS, edge 집합 동일, axiom `dynamic_call` 18→19(늘어난 1줄 `Keyword.put(opts, :resource_scope, scope)`는 런타임 MFA 모양이라 보수적으로 잡힌 오탐), taeguk `apps/taeguk`는 digest 비교로 `lib/taeguk/state.ex`가 마지막 compile 뒤 바뀐 것을 확인해 FAIL, 대상 `git status` 불변. 이전 기록: 18차 수정본 재실측: 6곳 PASS, edge 집합 동일, axiom `dynamic_call` 14→18(늘어난 4줄은 원본 대조 결과 모두 실제 콜백 호출 `fun.(tx)`), 대상 `git status` 불변. 이전 기록: 17차 수정본 재실측: 6곳 PASS, edge·gap 집합이 16차 수정본과 동일(그래프 파일은 새 디렉터리 핸들 쓰기로 기록), 대상 `git status` 불변. 이전 기록: 16차 수정본 재실측: 6곳 PASS, edge·gap 집합이 15차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 15차 수정본 재실측: 6곳 PASS, edge·gap 집합과 `dirty` 판정이 14차 수정본(git diff 기반)과 같아 filter 없는 작업 트리 비교가 같은 결과를 냄을 확인, 대상 `git status` 불변. 이전 기록: 14차 수정본 재실측: 6곳 PASS, 대상 `git status` 불변. thesmothetai·taeguk은 대상 저장소에 새 commit이 생겨 gap·edge가 늘었다(`importlib.util`을 쓰는 새 테스트 파일, `scripts/check_doc_drift.py → scripts/check_rules.py` 새 import). 나머지는 13차 수정본과 동일. 이전 기록: 13차 수정본 재실측: 6곳 PASS, edge 집합 동일. axiom `dynamic_call`은 파서 기반 검사로 18→14(줄어든 4줄은 원본 대조 결과 모두 문자열·sigil 속 점이던 오탐), 대상 `git status` 불변. 이전 기록: 12차 수정본 재실측: 6곳 PASS, edge 집합 동일, axiom `dynamic_call` 15→18(점 접근 허용 목록), 대상 `git status` 불변. 이전 기록: 11차 수정본 재실측: 도구를 ROOT 밖에서 실행해도 6곳 PASS, edge 집합 동일, akashic 동적 import gap 57→65(`sys.modules`·`exec` 등), 대상 `git status` 불변. 이전 기록: 10차 수정본 재실측: axiom Elixir edge가 kind까지 `mix xref` 결과와 동일(manifest reader로 교체 후), 6곳 PASS, edge·gap 집합은 9차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 9차 수정본 재실측: 분석기 격리 후에도 6곳 PASS, edge·gap 집합은 8차 수정본과 동일, 대상 `git status` 불변. 이전 기록: 8차 수정본 재실측: edge 집합은 7차 수정본과 동일. gap은 axiom `dynamic_call` 15(노드 10개 중 6개 파일, 맵 필드 접근 포함), akashic `construct_not_analyzed` 24개 파일(`json!` 16, serde derive·attribute, `include_str!` 4), `tools/taeguk-cli` 3개 파일 모두. akashic `core.py` 변경 impact의 `llm_review`는 110개. 이전 기록: 7차 수정본 재실측: edge 집합은 6차 수정본과 동일, gap은 akashic 동적 import 39→57·매크로 7→19(`json!` 16, `matches!` 4, `include_str!` 4, `vec!` 1), hoplites 4→6, taeguk 8→11, `tools/taeguk-cli` 매크로 1→3(`json!`). 이전 기록: 6차 수정본 재실측. akashic에 edge 1개 추가(`crates/akashic-engine/src/main.rs`의 `vec![...]` 안 `akashic_core::RelationKind`, 원본 48~60행 대조)와 `macro_body_not_analyzed` gap 7개, 동적 import gap 18→39, hoplites 2→4, taeguk 4→8, `tools/taeguk-cli` 매크로 gap 1. 나머지 edge 집합은 5차 수정본과 동일. 이전 기록: 5차 수정본(4차 수정본과 edge 집합 동일, axiom은 기존 untracked 파일 9개로 `dirty: true` 기록). ignored 경로를 엄격 검사한 첫 구현은 hoplites·axiom·akashic의 `.venv`·캐시 링크 때문에 FAIL해 ignored 경로 규칙을 특수 파일 거부로 좁혔다. akashic에서 `personal/akashic/core.py` 변경 impact는 영향 5개, `llm_review` 47개로 UNKNOWN: hoplites(Python) 노드 49·edge 73, thesmothetai(Go+Python) 13·23, axiom(Elixir+Python) 10·10, akashic 작업 트리(Rust+Python) 328·178, taeguk 최상위(Python) 11·1, `tools/taeguk-cli` 3·4 모두 PASS, 실행 전후 대상 `git status` 동일. akashic의 늘어난 4개는 `personal/akashic/{server,service,storage,workflow}.py`의 `from .core import`가 기준 패키지 `__init__.py`로 가는 edge로 원본과 대조해 확인. taeguk `apps/taeguk`는 main checkout `_build/dev`가 소스보다 오래되어 FAIL(정상 동작). 1차 때 무작위 edge 25개 수동 대조 |
| CG-V04 | PASS | 같은 commit 재실행 digest 동일, 파일 수정 후 `dirty: true`와 경로 기록·digest 변경, 기록된 python 버전이 `python3 --version` 출력과 일치 |
| CG-V05 | PASS | 수작업 영향 집합(Python 5개 노드·거리 0~3, Go test import 포함 4개 노드)과 일치, `--depth 1` 기록, stale·dirty·FAIL 그래프, 질의 시점 미커밋 변경·untracked 파일, ignored 입력 변경(`analyzer_inputs_changed`), 미대응 파일, 영향 범위 밖 gap(`llm_review`)에서 UNKNOWN과 이유, digest 변조 그래프 거부 |
| CG-V06 | PASS | 변경 스킬 2개 skill-creator validator 통과, 한계 문구 포함, project 설치 dry-run에 변경 파일 포함(apply는 하지 않음) |
| CG-V07 | PASS | `check_rules.py` PASS, Python 110개 OK, `cargo test --locked`(`TAEGUK_REQUIRE_GRAPH_TOOLS=1`) 52개 OK(Elixir 줄 단위 검사 단위 테스트 2개를 파서 기반 통합 사례로 대체), `cargo clippy --all-targets` 경고 0, `mix format --check-formatted` OK, DB 연결 `mix test` 21개 OK |
| CG-V08 | 진행 중 | 아래 리뷰 기록 참고 |

## 로컬 Codex 리뷰 기록

1차 대상 `b630b14...b9e6f61`, `codex exec review`(codex-cli 0.164.0, `gpt-6-astra`, `medium`, read-only, ephemeral), 2026-10-28T14:19~14:23Z. 실행 전후 HEAD·status·fingerprint 동일.

- 교차 리뷰: exit 0, 지적 8건(P1 1, P2 7). 판정과 처리:
  1. P1 Elixir xref가 오래된 빌드에서 자동 compile: 재현 가능(Mix `xref`는 `--no-compile`이 없으면 compile 실행). `--no-compile --no-deps-check`와 manifest 대비 stale 검사로 수정, 테스트 추가.
  2. P2 impact가 도구 버전 변경을 확인하지 않음: CG-R02 위반으로 판단. 현재 버전과 비교해 다르거나 확인 불가면 UNKNOWN, 테스트 추가.
  3. P2 다른 ROOT의 그래프 재사용: 타당. ROOT 불일치 시 UNKNOWN, 테스트 추가.
  4. P2 `os.path.join("logs", ...)` 같은 문자열 경로에서 TypeError로 Python 분석 전체 FAIL: 재현. 디렉터리 값만 받도록 수정, fixture에 해당 줄 추가.
  5. P2 상대 import가 중간 패키지 `__init__.py`를 빠뜨림: 타당. 수정, fixture에 `from .sub.leaf import` 추가.
  6. P2 Rust 외부 crate를 package 이름으로만 등록: 타당. library target 이름 사용, fixture를 package 이름과 lib 이름이 다르게 바꿈.
  7. P2 Rust glob import gap 미기록: 설계 문구와 구현이 달랐다. 파일 단위 그래프에서는 glob이 대상 모듈 파일 edge가 되고 재노출은 그 모듈의 edge로 이어져 누락이 생기지 않으므로 설계 문구를 바로잡았다.
  8. P2 Elixir 동적 호출 gap 없음: CG-R01 대비 타당. `dynamic_call` gap 추가, fixture 기대 gap 갱신.
- 보안 리뷰: exit 1, “Selected model is at capacity”로 결과 없음 → NOT_RUN. 수정한 새 head로 재실행한다.

수정 후 시험 프로젝트 재실측: taeguk `apps/taeguk`는 main checkout의 `_build/dev`가 최근 소스보다 오래되어 FAIL로 바뀌었다. 이전 PASS는 xref가 임시 사본에서 몰래 compile한 결과였다. 나머지 5곳은 수치가 이전과 같고 PASS다.

2차 대상 `b630b14...de94428`, 같은 설정, 2026-10-28. 실행 전후 HEAD·status·fingerprint 동일.

- 교차 리뷰: exit 0, 지적 6건(P1 1, P2 5). 보안 리뷰: exit 0, 지적 4건(P2). 상대 import 지적이 1차 5번 수정의 미완(기준 패키지 `__init__` 누락)이었으므로 공통 상향 조건에 따라 멈추고 BOSS에게 보고했다. BOSS가 “모두 수정 후 3차”로 답했다.
- 처리(모두 타당으로 판정):
  1. P1 `GOPRIVATE`·`GONOPROXY` 직접 다운로드(교차·보안 공통): 비우고 `GONOSUMDB` 비움, `GOSUMDB=off`, `GOVCS=*:off`.
  2. P2 rustup 자동 설치: `RUSTUP_AUTO_INSTALL=0`. 버전 기록 명령도 ROOT에서 같은 환경으로 실행해 분석에 쓰인 toolchain과 기록 버전을 맞췄다.
  3. P2 링크·특수 파일 무한 읽기: 모든 분석기가 실제 경로 ROOT 내부·일반 파일·8 MiB 이하만 읽는다. 수정 전 코드에서 재현됨(위 CG-V02).
  4. P2 `#[path]` 순환 재귀: 현재 `mod` 사슬의 canonical 파일이 다시 나오면 FAIL.
  5. P2 질의 시점 미커밋 변경: `working_tree_differs_from_head`로 UNKNOWN.
  6. P2 삭제된 Elixir 소스: xref 결과의 파일이 tracked·존재하지 않으면 FAIL.
  7. P2 Go test 전용 import 실패 누락: `-test`로 test 의존까지 적재하고 test 변형 패키지는 노드에서 제외.
  8. P2 Rust `use` 별칭 경로 누락: `use`가 묶은 이름을 원래 경로로 펼쳐 해석(깊이 8 제한).
  9. P2 상대 import 기준 패키지 `__init__` 누락: 포함.
- 수정 중 추가 발견: Mix manifest 시각이 초 단위라 compile과 같은 초에 저장된 소스가 더 새 파일로 오판되어 정상 빌드가 FAIL했다. 초 단위 비교로 수정하고 테스트 추가.

3차 대상 `b630b14...4114601`, 같은 설정, 2026-10-28T22:07~22:10Z. 교차·보안 모두 exit 0, 실행 전후 HEAD·status fingerprint 동일. P0·P1 0건, P2 9건(중복 제외). `mix.exs` 무제한 읽기가 2차 3번 수정의 누락이어서 멈추고 보고했고, BOSS가 “모두 수정 후 4차”로 답했다. 모두 타당으로 판정하고 처리했다.

- 안전·자원: `mix.exs` 안전 읽기, Python FIFO는 열기 전 파일 종류 확인과 nonblocking open, Go 입력 파일 ROOT 검사, Python `parents[N]` 인덱스 선검사, Rust 모듈 파일 방문 상한 10,000.
- 정확도: Rust cfg 동명 모듈 분기별 보존, `use` 별칭을 모듈·블록 scope로 한정, glob import를 통한 하위 모듈 해석(1차 7번의 “누락 없음” 판단이 틀렸음을 정정), Python `sys.path` 대입 gap.

4차 대상 `b630b14...2cdb901`, 같은 설정, 2026-10-28T22:19~22:21Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건, P2 8건(교차 6, 보안 2). 같은 부류 지적이 계속 새로 나와 멈추고 보고했고, BOSS가 “모두 수정 후 5차”로 답했다. 모두 타당으로 판정하고 처리했다.

- 안전·현재성: 개별 입력 나열 대신 ROOT 전체 직접 순회 검사, `_build` 복사 검사, untracked 파일을 dirty에 포함.
- 정확도: Rust 별칭을 선언 scope 기준으로 펼치고 cfg 분기별 바인딩 보존, 모듈 재노출·glob 재노출 추적, Python `sys.path` 변경의 문장 순서 평가.

5차 대상 `b630b14...ed09248`, 같은 설정, 2026-10-28T23:04~23:07Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건, P2 7건(교차 5, 보안 2). 상향 보고 중 BOSS가 태극 보완 원칙(정적으로 덮고 남은 영역은 LLM으로 덮음)을 지적했고, 위 분담 방식으로 진행을 확인했다.

- 안전·보안 3건 수정: `--out` 링크 추종 금지, ignore된 경로의 특수 파일 검사, `_build/dev` 시작 경로 검사.
- 정확도 4건은 gap 표시로 처리: 확장 상한 초과 gap, Python `sys`·동적 import 별칭 추적, Elixir 괄호 없는 호출 감지, crate를 넘는 재노출은 대상 crate 의존성으로 해석.
- impact에 `llm_review`를 추가하고 두 스킬 문서와 `docs/architecture.md`에 분담을 적었다.

6차 대상 `b630b14...3f381de`, 분담 판정 기준을 프롬프트에 포함, 2026-10-28T23:24~23:26Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건, P2 9건(교차 6, 보안 3). 모두 분담 기준으로도 결함(gap 없는 누락·현재성·안전)이었고, 사례별 수정이 같은 부류의 새 사례를 낳아 멈추고 보고했다. BOSS가 “부류 단위로 진행”으로 답했다.

- 현재성: 분석기 입력 전체의 해시 지문(`inputs`)을 기록하고 impact에서 비교.
- 검사 범위: 선택된 분석기가 읽지 않는 디렉터리만 제외(go가 있으면 `target` 포함).
- 출력: 임시 파일 생성 후 `rename`으로 교체.
- 해석 작업량: 방문 상태 중복 제거와 예산, 초과 시 gap.
- gap 일반화: Rust 매크로 인자 해석·해석 불가 매크로 gap·`cfg_attr` 경로 gap, Python 동적 import 기구 참조 gap, Elixir 변수 원격 호출 gap. 매크로 토큰 검사에 `proc-macro2`를 직접 의존으로 선언했다(이미 `syn`이 쓰는 1.0.107, 새 패키지 없음).

7차 대상 `b630b14...6ac24b1`, 같은 설정, 2026-10-28T23:50~23:52Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건, P2 5건(교차 4, 보안 1). 새 부류 없이 부류 수정의 경계 사례였고, 보고 후 BOSS가 “수정 후 8차”로 답했다.

- 검사 범위: 디렉터리 이름 대신 도구가 남긴 표시로 빌드 출력 판단.
- 별칭 확장 단계에도 작업 예산과 gap.
- 매크로: 표준 식 매크로만 분석 완료로 보고 나머지 매크로 호출은 항상 gap.
- Python: 속성·문자열로 쓴 동적 import 기구도 gap. Elixir: 괄호 없는 변수 원격 호출도 gap.

8차 대상 `b630b14...a94f039`, 같은 설정, 2026-10-29T00:21~00:23Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건, P2 7건(교차 6, 보안 1)으로 다시 늘었다. 원인이 못 보는 구문을 나열하는 방식에 있다고 보고했고, BOSS가 “허용 목록으로 전환”으로 답했다.

- Rust: 표준 매크로·내장 attribute·표준 derive 외 구문, 블록 안 `mod`, 파일 내부 attribute는 gap. 모듈 깊이 64 상한.
- Python: 함수·클래스 안의 `sys.path` 변경은 평가하지 않고 gap.
- Elixir: 변수·모듈 attribute 점 접근은 괄호·인자와 무관하게 gap.
- 입력 지문: 도구 표시로 확인한 빌드 출력만 제외하고 `target` 아래 소스도 포함.

9차 대상 `b630b14...0c7fde7`, 같은 설정, 2026-10-29T00:41~00:44Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건, P2 8건(교차 7, 보안 1). 10차를 마지막으로 두자는 제안에 BOSS가 “포기하면 다른 작업이 같은 문제를 다시 겪으니 합의점을 찾아 놓고 마무리하라”고 답했다. 그래서 위 결함 판정 기준을 정하고 부류를 구조로 닫았다.

- 자원: 분석기 자식 프로세스 격리와 시간 제한(파서 stack overflow, 남은 무한 대기 경로를 FAIL로 수렴).
- 허용 목록 노드 단위 적용: `use`의 attribute, `extern crate`, 표준 이름 매크로의 crate 내 재정의·`use` 바인딩·`#[macro_use]`, 중첩 `cfg_attr`, `Verbatim`.
- Python: 위치와 무관하게 함수·lambda·클래스 안의 `sys.path` 변경은 gap. Elixir: 주석을 잘라내지 않음.
- 링크: go가 순회하는 위치의 ignore된 ROOT 밖 디렉터리 링크 거부, cargo manifest의 ROOT 안 여부 사후 검사.

10차 대상 `b630b14...eb3f559`, 결함 판정 기준을 프롬프트에 포함, 2026-10-29T01:06~01:09Z. 교차·보안 모두 exit 0, fingerprint 동일. 교차 P2 3건, 보안 P1 1건.

- P1 `mix.exs` 실행: 그래프 생성이 대상 프로젝트 코드를 사용자 권한으로 실행했다(타당, 1차부터 “`mix.exs`는 실행된다”로 문서화만 했던 부분). `mix` 대신 compile manifest를 고정 스크립트로 읽도록 바꾸고, 판정 기준에 “분석기는 대상 프로젝트 코드를 실행하지 않는다”를 추가했다.
- P2: 분석기가 실제로 읽은 파일을 입력 지문에 포함(`read`), Python 패키지 우선, 표준 이름 derive 재바인딩 gap.

11차 대상 `b630b14...b1ce265`, 같은 기준, 2026-10-29T01:16~01:20Z. 교차·보안 모두 exit 0, fingerprint 동일. 교차 P2 6건, 보안 P1 1건·P2 1건.

- P1 toolchain 선택: 도구를 ROOT 안에서 실행해 `rust-toolchain.toml`의 `path`가 가리키는 저장소 안 프로그램이 실행될 수 있었다(이전 바이너리로 재현). 모든 도구와 버전 조회를 ROOT 밖에서 실행하도록 바꿔 부류를 닫고 판정 기준에 추가했다.
- P2: ROOT 검사 실패 시 도구 미실행, `pyproject.toml` 부재 기록, `exec`·`sys.modules` 등 gap, 상대 import 패키지 우선, 조건부 `sys.path` gap, `cfg_attr` derive와 외부 glob의 표준 이름 재바인딩 gap.

12차 대상 `b630b14...53e3104`, 같은 기준, 2026-10-29T01:25~01:29Z. 교차 exit 0, 실행 전후 fingerprint 동일, P2 3건(Python `builtins` 별칭 `exec`, 타입 주석 `sys.path` 별칭, Elixir 괄호 식 수신자). 보안 리뷰는 exit 1: 제공자가 요청을 사이버보안 위험으로 차단해 결과가 없음(NOT_RUN), 다음 회차에서 다시 실행한다. 사례 대신 규칙으로 닫았다: Python은 `sys`·`builtins` 사용을 허용 목록으로, Elixir는 점 접근 수신자를 허용 목록으로 판정.

13차 대상 `b630b14...ff36c62`, 같은 기준, 2026-10-29T01:33~01:36Z. 교차·보안 모두 exit 0(보안은 12차 NOT_RUN 뒤 재실행), fingerprint 동일. 교차 P2 4건, 보안 P1 2건.

- P1 manifest 역직렬화: `binary_to_term`으로 복원한 `Stream`을 열거하면 callback이 실행됐다(이전 바이너리로 재현). 읽는 부분만 열거 전에 plain data 모양으로 검사해 닫았다.
- P1 git 설정: checkout의 `core.fsmonitor`가 build·impact의 git 조회 중 실행됐다(재현). git을 `core.fsmonitor=false`와 빈 tree attribute로 실행해 fsmonitor·filter 경로를 닫았다.
- P2: `from builtins/sys import ...` gap, 식 안의 조건부 `sys.path` gap, Elixir 공백·줄바꿈 점 접근(줄 단위 검사를 Elixir 파서 기반 검사로 교체해 부류를 닫음), `std` 접두사 재바인딩 gap.

14차 대상 `b630b14...1f96706`, 같은 기준, 2026-10-29T01:44~01:47Z. 교차·보안 모두 exit 0, fingerprint 동일. 교차 P2 3건, 보안 P1 1건.

- P1 인자 주입: manifest의 source 이름이 `elixir` 명령줄 인자가 되어 `-r`이면 그 파일을 실행했다(이전 바이너리로 재현). 저장소 유래 문자열은 모두 stdin으로 넘기도록 바꾸고 판정 기준에 추가했다.
- P2: Python 경로 함수는 이 파일의 바인딩을 허용 목록으로 확인(재정의된 `dirname`은 gap), `resolve()`·`realpath`는 ROOT 안 링크를 따라감, Elixir `alias ..., as:`와 import된 지역 호출을 동적 호출 판정에 반영.

15차 대상 `b630b14...1cefaf6`, 같은 기준, 2026-10-29T01:53~01:57Z. 교차·보안 모두 exit 0, fingerprint 동일. 교차 P2 4건, 보안 P1 1건.

- P1 git filter: `--attr-source`는 `core.attributesFile`·`.git/info/attributes`를 막지 못해 clean filter가 실행됐다(이전 바이너리로 재현). git이 작업 트리 파일을 filter에 통과시키는 명령을 아예 쓰지 않도록 dirty 판정을 바꿔 부류를 닫았다.
- P2: Python 패키지 wildcard import gap, 상대 import는 경로 함수 바인딩에서 제외, Elixir 모듈명 정규화와 별칭 연쇄, Rust `::` 절대 경로 보존.

16차 대상 `b630b14...689fe31`, 같은 기준, 2026-10-29T02:04~02:09Z. 교차·보안 모두 exit 0, fingerprint 동일. 보안 리뷰는 기준에 해당하는 결함 없음(처음으로 0건). 교차 P2 3건: 프로젝트가 제공하는 `pathlib`, Elixir 별칭 재바인딩, manifest 링크 교체. 모두 반영했다.

17차 대상 `b630b14...042cc79`, 같은 기준, 2026-10-29T02:13~02:16Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 4건(wildcard 뒤 경로 함수, Elixir atom 별칭, `Elixir.` 절대 이름, Rust edition 2015), 보안 P2 1건(출력 상위 디렉터리를 링크로 바꾸는 경쟁). 모두 반영했다.

18차 대상 `b630b14...74cf935`, 같은 기준, 2026-10-29T02:22~02:26Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 3건(구조 분해 재대입, `__file__` 재대입, Elixir 익명 호출), 보안 P2 2건(그래프 입력 파일 종류·크기, 없는 노드로 가는 edge의 panic). 모두 반영했다.

19차 대상 `b630b14...b5c6656`, 같은 기준, 2026-10-29T02:30~02:36Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 5건(match 재바인딩, 모듈 속성 교체, `Function.capture`, manifest 초 단위 비교, `.test` 접미사 패키지), 보안 P2 1건(검사와 열기 사이 FIFO 교체로 부모 CLI 정지). 모두 반영했다.

20차 대상 `b630b14...071abc8`, 같은 기준, 2026-10-29T02:44~02:47Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. P2 5건(교차·보안 공통 blob 해시 메모리, 대입 우변 walrus, 링크 뒤 `..`, import한 `capture`, 소스 열기의 ROOT 경계 경쟁). 모두 반영했다.

21차 대상 `b630b14...628e77d`, 같은 기준, 2026-10-29T02:55~02:58Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. P2 4건(교차·보안 공통 입력 지문 읽기 상한, import 재바인딩, 식으로 계산된 MFA 모듈, 링크 blob 무제한 수집). 모두 반영했다.

22차 대상 `b630b14...aea2a27`, 같은 기준, 2026-10-29T03:02~03:08Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 3건(충돌 바인딩, 별칭 연쇄 상한, Go embed 입력), 보안 P2 1건(cargo 진단을 통한 ROOT 밖 내용 노출). 모두 반영했다.

23차 대상 `b630b14...75e82f6`, 같은 기준, 2026-10-29T03:13~03:16Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 3건(빌드 출력 안 Go embed 입력, `__path__` 확장, Go `plugin` 적재), 보안 P2 2건(go 오류의 ROOT 밖 패키지 이름, 임시 디렉터리 권한). 모두 반영했다.

24차 대상 `b630b14...61b9916`, 같은 기준, 2026-10-29T03:23~03:28Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 3건(경로 해석에 쓴 링크의 지문, 추가된 `sys.path`의 `pathlib`, 함수 이름이 런타임 값인 MFA), 보안 P2 1건(그래프 `read` 경로의 ROOT 이탈). 모두 반영했다.

25차 대상 `b630b14...7ea3928`, 같은 기준, 2026-10-29T03:31~03:36Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 2건(모듈 최상위 Go embed의 `read` 표현, pathlib `..` 조기 축약), 보안 P2 1건(부모 프로세스 git 호출에 시간 제한 없음). 모두 반영했다. 시간 제한을 모든 프로세스로 넓히면서 도구가 분석기와 다른 그룹에 들어가 분석기 시간 초과 때 cargo가 남는 회귀를 만들었고, 실측으로 발견해 그룹 정리를 CLI 프로세스 소유로 바꿨다.

26차 대상 `b630b14...9c680e2`, 같은 기준, 2026-10-29T03:51~03:56Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 2건(namespace 하위 패키지의 상위 `__init__` edge, Rust `#[path]`의 `..` 조기 축약), 보안 P2 1건(ROOT 밖 `go.mod` 링크의 module 경로 유입). 모두 반영했다. 보안 건은 분석기 입력 전체로 넓혀, 입력 파일이 ROOT 밖으로 가면 모든 분석기를 FAIL로 둔다.

27차 대상 `b630b14...3422a68`, 같은 기준, 2026-10-29T04:05~04:09Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 1건(Rust raw identifier), 보안 P2 1건(ROOT 밖 workspace 패키지 이름의 오류 노출). 모두 반영했다. Rust는 소스에서 읽는 모든 식별자(모듈 파일, 경로, 별칭, 매크로·attribute 이름)를 한 함수로 정규화하고, Go는 main module 패키지마다 읽은 파일 전체를 한 번에 검사해 ROOT 밖 경로나 ROOT 안에서 이름이 확인되지 않는 패키지는 이름 없이 보고한다.

28차 대상 `b630b14...520dd0b`, 같은 기준, 2026-10-29T04:14~04:19Z. 교차·보안 모두 exit 0, fingerprint 동일. P0·P1 0건. 교차 P2 2건(Rust 모듈 경로의 디렉터리 링크 지문, Python 모듈 `__dict__` 우회), 보안 P2 1건(go가 실행 중 읽은 파일의 교체 후 원복). 모두 반영했다. 보안 건은 기준 문서가 "실행 전·후 검사로 다룬다"고 적어 둔 경계였지만, 도구가 실제로 읽은 바이트를 묶는 방법(분석 전후 ROOT 상태 비교)이 있어 설계 경계 대신 결함으로 판정해 닫았다.

29차 대상 `b630b14...ba40003`, 같은 기준, 2026-10-29T04:30~04:34Z. 교차·보안 모두 exit 0, fingerprint 동일. 보안 P1 1건(promisor remote의 지연 fetch가 저장소 지정 전송 명령을 실행), 교차 P2 2건(`from builtins/sys import __dict__`, 큰 입력의 mtime 복원). 모두 반영했다. P1은 실제로 표지 파일이 생기는 것을 확인한 뒤, CLI가 시작하는 모든 프로세스(분석기 안의 go·cargo가 부르는 git 포함)에 `GIT_NO_LAZY_FETCH=1`·`GIT_ALLOW_PROTOCOL=`(모든 전송 금지)·`GIT_TERMINAL_PROMPT=0`·`GIT_OPTIONAL_LOCKS=0`을 걸어 닫았다. git diff는 `--no-ext-diff --no-textconv`, go는 `-buildvcs=false`도 쓴다.

같은 구현자가 fixture와 기대값을 함께 만들었으므로 CG-V01·V05는 같은 가정을 공유한다. 시험 프로젝트 수동 대조(CG-V03)가 다른 관측 층이다. CI에서 Go·Elixir·Python toolchain으로 같은 테스트가 도는지는 PR 실행 후 확인한다.

## BOSS 확정 (2026-10-28)

Claude Code 세션에서 BOSS가 아래 네 항목의 권장안에 “권장안대로 확정”이라고 답했다.

1. 목적 가설과 CG-R01~R06의 의미를 위 표대로 승인한다.
2. CG-R01은 모듈·패키지 의존 그래프로 시작한다. 함수 호출 그래프는 이번 수용 기준에 넣지 않는다.
3. CG-R05는 프로젝트별 수동 설치로 시작한다. installer 포함은 효과 관측 후 별도로 정한다.
4. 1계층(CG-R01~R04)을 먼저 전달하고 2계층(CG-R05~R06)을 이어서 진행한다.

CG-R03의 간접 영향 추적 깊이 기본값은 요구 의미가 아니라 설계 선택으로 보고 Phase 1에서 정한다. 결과에는 적용한 깊이를 함께 기록한다.
