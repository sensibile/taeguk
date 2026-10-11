# 서적 검토의 반례 실험

2026-10-05에 실행한 자체 작성 교육용 실험이다. 책의 코드를 복사하거나 실제 제품을 구현한 것이 아니다. 표준 라이브러리만 사용하며, DB는 임시 디렉터리에 만들고 실행 후 정리한다. 네트워크나 사용자 데이터는 사용하지 않는다.

```sh
cd /Users/tonton/Documents/study
python3 experiments/book_review_probes.py
```

기록된 결과는 [book_review_results.json](book_review_results.json)이다. Python 3.13.16, SQLite 3.53.1에서 실행했다.

| 실험 | 결과 | 말할 수 없는 것 |
|---|---|---|
| 불필요한 원소를 추가한 unique 결과 | 121개 입력 모두 약한 속성 통과, 강화한 속성은 모두 거부 | 모든 입력·구현의 정확성, AssertJ나 jqwik 실제 동작 |
| 추출 경계의 정수 변환 | 12.0은 오류를 숨기고 12.5는 드러냄 | Java 컴파일러 동작 검증 |
| 조건 조회 후 쓰기의 순서 열거 | 6개 가능한 순서 중 4개에서 공동 불변식 위반 | 실제 실패 확률, 특정 DB의 격리 보장 |
| 완료 기록과 효과의 커밋 경계 | 원자 처리의 4개 중단 지점에서 재실행 후 1회 효과; 분리 커밋은 효과 누락 | 브로커·외부 효과·다중 소비자·전원 손실의 안전성 |

마지막 실험은 자식 프로세스를 `os._exit`로 종료하고 DB를 다시 연다. 커밋 이전과 이후를 구분하지만 운영체제나 저장 장치의 장애를 주입한 것은 아니다. 정상 실행에서 종료 코드 73은 의도한 주입이고, 부모 프로세스는 예상 종료 코드와 결과를 확인한다. 그 외 종료 코드나 결과 불일치는 실험을 실패시킨다.

검토와 적용 판단은 [적용요소 정밀검토](../적용요소-정밀검토.md)를 참고한다.


## Java 언어 의미 최소 재현

```sh
java experiments/LegacyLanguageProbe.java
```

[자체 작성 코드](LegacyLanguageProbe.java)를 Java 25 환경에서 실행했고 종료 코드는 0이었다. 출력은 다음과 같다.

```text
declared-but-unthrown checked exception: compiled
double zero / int zero: NaN; no ArithmeticException
```

checked exception을 선언하고 실제로 던지지 않는 메서드가 컴파일되며, double 0을 int 0으로 나누면 NaN임을 확인한다. 정수 나눗셈이나 모든 Java/C++ 예제의 의미를 검증한 것은 아니다. 앞의 정수 변환 Python 실험과 별개의 실행이다.
