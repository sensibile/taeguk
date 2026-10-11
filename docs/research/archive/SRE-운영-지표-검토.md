# SRE 운영 지표와 관련 도서

2026-10-10 · 적용안은 승인 전 제안

운영 검증은 사용자에게 제공한 결과, 문제의 원인, 변경과 복구 과정, 측정 자체의 신뢰성을 구분해서 설계한다. 에이전트에게 지표를 주는 것만으로는 부족하다. 대상·시간창·분모·품질 조건과 지표에 따라 허용되는 행동을 함께 정의해야 한다.

## 관련 도서와 읽을 구간

우선순위는 이번 목적에 대한 판단이다. Google 두 권은 아래 명시한 관련 본문 구간을 확인했다. 나머지 세 권은 출판사 소개와 목차를 확인한 후보이며 본문 검토 완료로 표시하지 않는다.

| 우선순위 | 책과 확인 근거 | 이번에 얻을 내용 | 선택해서 읽을 범위 |
|---|---|---|---|
| 1 | [The Site Reliability Workbook](https://sre.google/workbook/table-of-contents/) | 운영 품질을 측정하고 경보·배포 판단에 연결하는 실제 방법 | 2장 SLO, 4장 모니터링, 5장 SLO 경보, 6장 toil, 16장 canary. 장애 대응은 9·10장 추가 후보 |
| 2 | [Implementing Service Level Objectives — Alex Hidalgo](https://www.oreilly.com/library/view/implementing-service-level/9781492076803/) | 지표의 의미·구현·통계·변경 조건 | 3·4장 SLI/SLO, 7·8장 측정/경보, 9장 통계, 11장 데이터 신뢰성, 14장 진화, 17장 보고 |
| 3 | [Observability Engineering, 2nd Edition — Charity Majors, Liz Fong-Jones, George Miranda](https://www.oreilly.com/library/view/observability-engineering-2nd/9781098179915/) | 지표로 드러난 문제를 이벤트·계측으로 조사하고 운영 피드백을 개발에 연결 | 목차에서 확인한 1장 관측성, 2장 운영에서 의도 검증, 4장 계측부터 시작. 이후 조사 방법 관련 장을 선택 |
| 기초 참조 | [Site Reliability Engineering](https://sre.google/sre-book/table-of-contents/) | 신뢰성 판단의 배경과 모니터링 원칙 | 4장 SLO, 6장 분산 시스템 모니터링. 실무 예시는 Workbook으로 보완 |
| 보완 | [Practical Monitoring — Mike Julian](https://www.oreilly.com/library/view/practical-monitoring/9781491957349/) | 모니터링 안티패턴, 경보, 통계, 사용자 관점의 계측 | 1·3·4장과 6·7장. 2017년 책이므로 도구와 프런트엔드 세부 사항은 최신 문서로 재확인 |

Observability Engineering 2판은 현재 출판사 목록에 2026년 6월 판으로 표시된다. 여기서는 목차·소개에 근거해 후보로 선정했으며 에이전트 적합성을 입증하는 책으로 간주하지 않는다. Google SRE 두 권은 [공식 사이트](https://sre.google/books/)에서 무료로 읽을 수 있다.

먼저 Workbook에서 지표·경보의 골격을 잡고, Implementing Service Level Objectives에서 통계와 데이터 지표를 보완한 뒤, Observability Engineering으로 원인 조사와 계측을 연결하는 순서를 제안한다. 기존 Systems Performance의 USE/RED 및 Chaos Engineering의 장애 실험은 계속 활용한다.

## 사용자 품질 지표

SLI는 측정값, SLO는 목표다. 관찰하려는 서비스 결과와 실제 측정 위치를 따로 정한다. 서버 로그는 서버에 도달하지 못한 요청을 놓칠 수 있으며, 외부 probe와 실제 사용자 계측도 서로 다른 사각지대를 가진다. [Workbook 2장](https://sre.google/workbook/implementing-slos/)

| 대상 | 지표 후보 | 판정에서 정할 것 | 교차 확인 제안 |
|---|---|---|---|
| 요청 처리 | 성공한 적격 요청 / 전체 적격 요청 | 성공의 의미, 거부 요청·재시도·시간초과 포함 방식 | 외부 요청 결과와 실제 업무 효과 |
| 응답 시간 | 승인된 시간 안에 완료한 적격 요청 비율 | 시작과 끝, 성공/실패 지연 분리 | 사용자 계측과 서버·하위 호출 지연 |
| 데이터 최신성 | 허용된 나이 이내 데이터를 제공한 비율 | 원천 시각, 접근 시점, 시계 오차 | 원천 갱신 기록과 읽기 결과 |
| 파이프라인 | 정확성·처리 범위·완료 기한 충족 | 입력 집합, 누락·중복·미완료 | 원천과 산출물 대조 |
| 저장 | 내구성·읽기 정확성 | 보존 약속과 손실 판정 | 저장 영수증·내용 대조·복구 실험 |

전체 평균이 정상이어도 중요한 사용자 집단에 문제가 있을 수 있다. 승인된 중요 경로와 서비스별 결과를 따로 본다. 많은 차원을 무제한으로 계측하기보다 실제 위험과 조사 목적에 필요한 구분을 선택한다.

## 에러 버짓과 소진율

이벤트 비율형 SLO에서 허용 불량 비율은 `1 - SLO`다. 에러 버짓 소진율(burn rate)은 `관측 불량 비율 / 허용 불량 비율`이다. 장·단기 시간창을 함께 확인하면 지속되는 큰 문제와 이미 회복된 문제를 구분하는 데 도움이 된다. 경보는 정밀도·재현율·탐지 시간·해제 시간으로 검토한다. 저트래픽에서는 한 번의 실패가 비율을 크게 바꾼다. [Workbook 5장](https://sre.google/workbook/alerting-on-slos/)

계산 예시: SLO 99.9%, 같은 평가 구간의 적격 요청 100,000건 중 불량 200건이면 허용 불량은 100건, 소비는 200%, burn rate는 2다. 이는 설명용 수치이며 제품 기준이 아니다. 요청 수가 변하면 사건 비율형 버짓의 절대 건수도 변한다. 요청 기반 버짓을 가동 중단 시간으로 바로 환산하지 않는다.

에이전트 적용 제안:

- 분모가 0이면 정상 비율을 만들지 않고 관측 없음으로 표시한다.
- 관측 부족과 실제 저트래픽을 구분한다. 합성 요청은 실제 요청과 구분해 보존한다.
- 에러 버짓은 우선순위와 승인된 대응 정책의 입력이다. 데이터 손실·접근 위반 같은 개별 불변식 위반을 버짓 여유로 허용하지 않는다.
- 장·단기 시간창과 임계값, 배포 제한·복구 행동은 승인된 규칙에 따른다. 책의 예시 수치를 자동 채택하지 않는다.

## 원인 조사와 용량 지표

Google의 네 가지 golden signals는 지연·트래픽·오류·포화다. 성공과 실패의 지연을 구분하고, 상태 코드가 정상이어도 잘못된 결과일 수 있음을 고려한다. 내부 계측과 외부 관찰은 서로 보완한다. [SRE 6장](https://sre.google/sre-book/monitoring-distributed-systems/)

기존 USE/RED 지침과 연결할 조사 후보는 큐 길이뿐 아니라 가장 오래 대기한 작업의 나이, 처리 속도와 유입 속도의 차이, 연결 풀 대기, 디스크 성장, 재시도 증폭, 자원 제한이다. 이는 제품 특성에 맞춰 선택할 제안이다. CPU 상승과 오류의 동시 발생만으로 인과를 확정하지 않는다. 트레이스·실행 기록·제한된 재현으로 원인 후보를 좁힌다.

## 배포 과정 지표

현재 DORA 공식 정의는 다섯 가지다. 이들은 서비스별 전달 성과이며 개별 변경의 정확성을 판정하는 검사나 개인·에이전트 순위 지표로 대체하지 않는다. [DORA 공식 지침](https://dora.dev/guides/dora-metrics/)

| 지표 | 정의 |
|---|---|
| Change lead time | 버전 관리에 커밋된 변경이 운영에 배포되기까지 시간 |
| Deployment frequency | 일정 기간 운영 배포 횟수 또는 배포 사이 시간 |
| Failed deployment recovery time | 즉각 개입이 필요한 실패한 배포에서 회복하는 시간 |
| Change fail rate | 배포 뒤 즉각 개입이 필요한 배포의 비율 |
| Deployment rework rate | 운영 사고 때문에 발생한 비계획 배포의 비율 |

구현 제안: 커밋·아티팩트·배포·사고를 연결하는 기록을 먼저 만든다. ‘운영 배포 완료’와 ‘회복’의 시점을 정의한다. 모든 원인의 장애 복구 시간을 failed deployment recovery time에 섞지 않는다. 서비스별 추세를 비교하고 배포 횟수만 늘리는 목표를 주지 않는다.

개별 배포에서는 SLI, 대표성 있는 표본, 관찰 기간, canary/control 구분이 중요하다. 단순 전후 비교는 시간대와 부하 변화의 영향을 받는다. 전체 지표는 작은 canary의 악화를 숨길 수 있고, 공유 의존성은 control도 오염시킬 수 있다. [Workbook 16장](https://sre.google/workbook/canarying-releases/)

## 장애 대응과 반복 운영 비용

추가로 제안하는 사고 기록은 영향 시작·탐지·대응 시작·영향 완화·서비스 회복·데이터 복구·재발 여부다. ‘MTTR’ 하나로 뭉치지 않고 시작/종료의 의미와 사건별 분포를 보존한다. 열린 사고는 완료된 것으로 계산하지 않는다. 알림 수가 줄었는지는 미탐지 사고와 함께 평가한다.

Toil 개선은 반복 작업을 수행하는 비용과 이를 없애는 개발·유지 비용을 함께 본다. 자동화가 원인 제거를 대신하면서 문제를 고착시키지 않는지 확인한다. [Workbook 6장](https://sre.google/workbook/eliminating-toil/)

에이전트 루프에는 별도 지표를 제안한다: 같은 문제의 반복 출현, 자동 수정 뒤 재발, 롤백 비율, 사람 개입 원인, 검증 시간·자원 비용, 운영 결과 확인까지 걸린 시간. 작업 수나 자동화율만으로 성공을 판정하지 않는다. 수정 후 SLI와 데이터 효과를 확인해야 루프가 닫힌다.

## 에이전트가 사용할 측정 계약

운영 도구의 API 유무와 지표 판정의 신뢰성은 별도로 검토한다. 예를 들어 [Prometheus HTTP API](https://prometheus.io/docs/prometheus/latest/querying/api/)는 JSON 결과와 오류·경고, 시점·구간 조회를 제공한다. API 호출 성공은 서비스 품질 통과를 뜻하지 않는다.

아래는 도구 선택과 무관하게 사용할 측정 계약 제안이다.

```yaml
metric:
  id: <metric identifier>
  service_scope: <service, environment, critical journey>
  specification: <user outcome being measured>
  implementation: <source and versioned query>
  good_event: <approved definition>
  eligible_event: <denominator and exclusions>
  window: <start, end, timezone, step>
  target_rule: <approved target and version>
quality:
  observed_count: <actual samples or eligible events>
  missing_intervals: <gaps>
  freshness: <last observation time>
  warnings: <query or collection warnings>
  comparability: <artifact, workload, population differences>
result:
  value: <measurement or unavailable>
  status: <PASS, FAIL, UNKNOWN, NOT_RUN, NOT_APPLICABLE>
  evidence: <raw result and linked logs or traces>
action:
  allowed: <approved read, reproduce, fix, deploy or recover scope>
  stop_condition: <approved limit>
```

빈 결과, 오래된 결과, 일부 누락된 결과를 정상으로 처리하지 않는다. 측정 API의 경고를 보존하고, 관측 상태와 서비스 품질 판정을 구분한다. 대시보드 이미지에만 의존하지 않고 동일 조건의 조회와 원시 결과를 증거로 남긴다.

## 확인 범위

Google 공식 본문은 Workbook 2장의 SLI 정의·측정 위치·대표적 지표, 5장의 경보 평가·burn rate·복수 시간창·저트래픽, 6장의 toil 측정, 16장의 지표 선택·대표성·원인 귀속을 중심으로 확인했다. SRE 6장은 증상/원인, 내부/외부 관측, golden signals를 확인했다. 장 전체·전체 그림·모든 예제의 읽기 완료와는 구분한다. DORA 공식 정의와 Prometheus API의 응답 형식·조회 조건도 확인했다.

후보 세 권은 목차·소개 확인 단계이며 이 근거로 읽을 구간을 골랐다. 실제 제품의 SLO와 계측·자동 행동은 아직 정하지 않았다. 기존 [검증 도구 선택 지침](검증-도구-선택-지침.md)과 함께 제품 조건에 맞게 후보를 좁힌다.


## 지표별 측정과 검증 보완

[검증 및 운영 지표의 측정과 검증 설계](지표-측정-검증-설계.md)에서 사건 단위·수집·계산·측정기 검증·판정 한계를 지표별로 연결했다. 실제 적용 시 이 상세 설계와 함께 확인한다.
