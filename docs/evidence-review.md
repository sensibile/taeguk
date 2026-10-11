# 완료 주장과 증거 연결

이번 보완은 현행 REQ-001·REQ-006·VER-003·VER-004를 적용하는 작업 방법과 선택적 도구다. 승인 규칙·상태 의미·통과 임계값을 바꾸지 않는다. 문서와 도구는 실행 권한이나 승인 권한을 부여하지 않는다.

## 적용할 때

분석·제품 과업을 마감할 때 원래 사용자 질문에 대한 답과 처리 정확성을 따로 검토한다. 테스트 수나 역할 호출 수로 답의 충분함을 판단하지 않는다. [완료 검토 참고](../codex/skills/taeguk-work-contract/references/completion-review.md)는 영향이 큰 의문을 조사하고 남은 확인 부담을 정리하는 방법이다.

Midas 파일럿의 후속 검토에서 집계와 수집 검증은 있었으나 반복 금융 항목의 해석, 중요한 질문의 압축, 반복 수집의 마찰 해결은 부족했다. 이 판단은 BOSS의 품질 불만과 전달된 보고서·코드를 검토한 결과다. 독립 사용자 연구나 품질 개선 측정은 아니다. 과거 분석을 다시 수행하거나 금융 정보를 이 저장소로 가져오지 않았다.

## CLI 입력

```sh
taeguk audit tools/taeguk-cli/examples/evidence-review/review.json tools/taeguk-cli/examples/evidence-review
```

[예제 입력](../tools/taeguk-cli/examples/evidence-review/review.json)은 가상의 작업이다. 예제 로그는 실제 검사 결과가 아니며 예제의 일치는 제품 검증으로 사용하지 않는다. 실제 작업에는 실행 로그·관측 기록을 별도로 저장하고 hash를 넣는다. 파일명과 로그에 개인정보·secret이 있으면 개인 로컬 디렉터리에서 관리한다.

| 필드 | 의미 |
| --- | --- |
| schema_version | 현재 1 |
| target_version, criteria_source | 대상 버전과 승인된 기준의 출처. 선언의 진실성과 승인은 도구가 확인하지 않음 |
| artifacts | 대상 파일의 path·sha256 배열. 비어 있으면 artifacts_match=false |
| checks | id·status·reason·oracle·command·evidence 배열. command는 문자열 또는 null, 실행하지 않음 |
| requirements | id·expectation·status·check_ids 배열. 해당 검사들을 묶어 요구에 연결 |
| open_questions | id·impact·investigation·next_action 배열. 미결 항목은 조사 내용과 다음 행동을 기록 |
| effect_status, effect_reason | 실제 효과의 선언 상태와 이유. 계산 검사와 별도 보존 |

모든 필드는 필수이며 알 수 없는 필드는 거절한다. ID는 영문·숫자·밑줄·하이픈, 각 배열 내 ID는 유일해야 한다. 상태는 PASS·FAIL·UNKNOWN·NOT_RUN·NOT_APPLICABLE이다. 이유·기대 결과·oracle·조사 내용은 공백만으로 채울 수 없다. 질문의 조사 내용에는 조사하지 못한 경우 그 이유를 기록할 수 있다. 이 형식 검사가 조사의 깊이를 판단하지는 않는다.

모든 파일은 ROOT 안의 일반 상대 경로이며 경로 탈출·심볼릭 링크를 거절한다. 검사별 evidence는 path·sha256 배열이다. JSON은 1 MiB까지 읽는다. 대상 파일과 증거 파일은 스트리밍 hash로 대조하며 기존 verify와 같이 신뢰할 수 있고 변경되지 않는 로컬 파일을 전제로 한다. 증거 파일 누락·잘못된 경로는 입력 오류다.

## 출력의 해석

artifacts_match는 대상 파일이 모두 맞는지, evidence_linked는 대상과 해당 검사 증거 파일이 모두 맞는지 나타낸다. PASS·FAIL 검사에 일치하는 파일이 없으면 gap을 출력한다. pass_claim_linked는 요구의 선언 상태가 PASS이고 연결된 검사가 하나 이상이며 모두 선언 PASS·파일 일치일 때만 true다. 요구가 PASS인데 근거가 빠졌거나 NOT_RUN 등이 연결되면 gap이 남는다.

이는 참조 연결만 보여 준다. 파일 일치만으로 검사 실행·기준 충족·해석 타당성·독립성·실제 효과를 확인할 수 없으므로 전체 완료 verdict는 출력하지 않는다. 파일 변경은 연결을 끊지만, target_version 문자열만 바꾸거나 관련 없는 로그를 넣는 문제는 의미 검토로 확인해야 한다. 악의적인 동시 교체 방지나 서명 인증은 제공하지 않는다.

정상 JSON 출력은 gap이 있어도 exit 0이다. 입력 오류는 JSON stderr·exit 2다. shell의 성공코드를 제품 통과로 해석하지 않는다. 기존 measure·verify 입력과 판정은 유지했다. 자동 gate나 전역 필수 기준으로 등록하지 않았다.
