"""한국어 학습 화면과 문서를 생성한다. 외부 패키지가 필요 없다."""
import html
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BASE = 'https://learning.oreilly.com/library/view/tidy-first/9781098151232/'
esc = html.escape
rows = []
for line in (ROOT/'study.tsv').read_text().splitlines():
    n, title, summary, question, answer = line.split('\t')
    n = int(n)
    rows.append(dict(id=n, title=title, summary=summary, question=question,
                     answer=answer, part=1 if n <= 15 else 2 if n <= 21 else 3,
                     page=f'ch{n:02d}.html'))
assert [r['id'] for r in rows] == list(range(1,34))
scope = '2026년 10월 5일 로그인된 O’Reilly 브라우저에서 추천사와 1~33장 본문을 확인했다. 머리말·도입부·각 부의 도입·부록·색인은 정리 범위에 포함하지 않았다. 그림은 재현하지 않았다. 장별 요지는 짧은 한국어 재구성이며 전체 번역본이 아니다. 복습 질문·답·학습 순서·예제·실험 양식은 별도로 작성했다.'
intro = '다음 변경을 쉽게 만드는 만큼, 작게 정리한다.'
parts = ['작은 코드 정리', '정리 작업의 관리', '설계를 판단하는 관점']
sessions = [
('1회차 · 흐름과 이름', '1~10장', '실제 함수 하나에서 이해를 방해하는 조건이나 이름을 찾는다. 다음에 필요한 변경을 한 문장으로 적는다.'),
('2회차 · 경계와 설명', '11~15장', '분리하면 좋은 부분과 모으면 좋은 부분을 각각 찾아 이유를 적는다.'),
('3회차 · 변경 단위', '16~20장', '최근 변경 하나를 구조 변경과 동작 변경으로 분류한다. 검토자가 확인할 차이를 설명한다.'),
('4회차 · 정리 시점', '21장', '현재 작업에 먼저·직후·나중·하지 않기 중 하나를 선택하고 선택이 바뀔 조건을 적는다.'),
('5회차 · 가치와 복구', '22~28장', '현재 이익과 미래 선택을 구분한다. 기대한 이익의 가정과 되돌리는 방법을 적는다.'),
('6회차 · 변경의 전파', '29~33장', '변경 하나가 전파되는 요소를 나열하고 최소 정리 한 가지를 실험한다.')]
experiment = '''# 작은 정리 실험 기록

학습용 독립 양식이다. 복사해서 별도 파일에 작성한다.

- 다음에 필요한 동작 변경:
- 지금 이해하거나 수정하기 어려운 부분:
- 선택한 시점: 먼저 / 직후 / 나중 / 하지 않기
- 선택 이유와 아직 확인하지 못한 가정:
- 이번에 할 구조 변경 한 가지:
- 이번에 하지 않을 일:
- 보존해야 할 결과·오류·부작용·실행 순서:
- 변경 전 확인 결과:
- 변경 후 확인 결과:
- 되돌리는 방법과 복구되지 않는 외부 영향:
- 예상한 효과:
- 실제로 쉬워진 부분 또는 늘어난 부담:
- 다음 판단을 바꿀 근거:

예시: 배송비 계산에 새 지역 조건을 넣어야 한다. 먼저 중첩 조건만 정리한다. 경계 무게의 결과와 오류 처리를 보존한 뒤 별도 단계에서 새 조건을 추가한다. 함수가 짧아졌는지보다 변경 위치와 확인해야 할 사례가 명확해졌는지 기록한다.
'''
(ROOT/'실험기록.md').write_text(experiment)
md = ['# Tidy First? 한국어 학습 노트', intro, scope,
      '[학습 화면](index.html) · [실험 기록](실험기록.md)',
      '## 공부하는 방법', '장별 요지를 읽고 답을 가린 채 질문에 답한다. 답을 외우기보다 자기 코드의 사례로 설명한다. 요지는 책에 근거하며 질문과 답은 독립적으로 작성한 적용 해설이다.',
      '## 학습 순서']
for title, chapters, task in sessions:
    md += [f'### {title}', f'{chapters} — {task}']
cards = []
for r in rows:
    n = r['id']
    if n in (1,16,22):
        md += [f'## {r["part"]}부 · {parts[r["part"]-1]}']
    md += [f'### {n}장 · {r["title"]}', r['summary'], f'[원문]({BASE}{r["page"]})',
           f'**복습 질문**: {r["question"]}', f'**학습용 해설**: {r["answer"]}']
    cards.append(f'''<article class="practice" id="p{n}" data-part="{r['part']}">
<div class="card-top"><span class="eyebrow">{r['part']}부 · {n:02d}장</span><label class="understood"><input type="checkbox" data-done="{n}"> 이해했어요</label></div>
<h3>{esc(r['title'])}</h3><p class="summary">{esc(r['summary'])}</p>
<div class="question"><h4>복습 질문 · 별도로 작성</h4><p>{esc(r['question'])}</p><details><summary>해설 보기</summary><p>{esc(r['answer'])}</p></details></div>
<details class="memo"><summary>내 사례와 생각</summary><label for="note{n}">내 코드의 사례와 적용 조건</label><textarea id="note{n}" data-note="{n}" rows="3"></textarea></details>
<a class="source" href="{BASE}{r['page']}" target="_blank" rel="noopener noreferrer">{n}장 원문 ↗</a></article>''')
code_guide = '직접 만든 배송비 계산 예제로 보호 구문과 설명 변수를 연습한다. 비교 범위는 정수 무게와 참·거짓 지역 표시에 한정한다. 잘못된 조기 반환이 감사 기록을 누락하는 반례도 포함한다. 이 테스트는 모든 입력이나 운영 환경에서의 동등성을 증명하지 않는다.'
md += ['## 코드 연습', code_guide, '실행: `python3 examples.py`',
       '예상 결과: `17 checks passed; incorrect guard clause detected`',
       '과제: 먼저 기존 동작을 보존하는 정리만 한다. 그다음 별도 변경으로 무료 배송 조건을 추가하고 경계 사례를 추가한다.',
       '## 오해하지 않기', '정리는 무조건 먼저 하는 의식이 아니다. 결합도는 특정 변경에 대해 판단한다. 책의 경제적 설명과 비용 도식은 정확한 예측식이 아니다. 작은 구조 변경도 실제로 동작을 바꿨다면 정리의 의도를 달성한 것이 아니다. 검토 생략에 대한 책의 제안은 안전성과 신뢰를 전제로 한 실험이며 팀의 정책을 대신하지 않는다.']
(ROOT/'학습노트.md').write_text('\n\n'.join(md)+'\n')
css = (ROOT/'style.css').read_text()
script = '<script>' + (ROOT/'controls.js').read_text() + '</script></body></html>'
toc = ''.join(f'<a href="#p{r["id"]}" data-jump="{r["id"]}">{r["id"]:02d} {esc(r["title"])}</a>' for r in rows)
plan = ''.join(f'<li><strong>{t}</strong><span>{c}</span><p>{x}</p></li>' for t,c,x in sessions)
filters = ''.join(f'<button data-filter="{i}" aria-pressed="false">{i}부 {title}</button>' for i,title in enumerate(parts,1))
body = f'''<!doctype html><html lang="ko"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Tidy First? 한국어 학습 노트</title><style>{css}</style></head><body>
<a class="skip" href="#main">본문으로 이동</a><div class="shell"><aside><div class="brand">Tidy First?</div><p class="aside-label">한국어 학습 노트</p><nav><a href="#plan">6회차 학습 순서</a><a href="#practices">33장 핵심 요지</a><a href="#examples">코드 연습</a></nav><div class="toc">{toc}</div></aside>
<main id="main"><header class="hero"><span class="tag">읽기 → 설명하기 → 작게 바꾸기</span><h1>정리부터 할까?</h1><p class="lead">{intro}</p><p class="meta">Kent Beck · O’Reilly · 원문 확인 2026년 10월 5일</p><div class="hero-links"><a href="학습노트.md">한국어 노트</a><a href="실험기록.md">실험 기록 양식</a><a href="{BASE}">원서</a></div></header>
<section class="panel"><h2>공부하는 방법</h2><p>핵심 요지를 읽고 질문에 먼저 답하세요. 자기 코드의 사례로 설명할 수 있으면 이해 표시를 하세요. 한 번에 하나만 실험하고 실제 효과를 기록하세요.</p><p class="small">각 카드의 첫 문단은 책의 요지입니다. 질문과 해설은 학습용으로 별도 작성했습니다.</p><details><summary>확인 범위</summary><p>{scope}</p></details></section>
<section class="panel"><h2>시점을 판단하는 네 가지 선택</h2><p><strong>먼저:</strong> 지금의 이해나 다음 변경을 쉽게 만드는 작은 정리가 보일 때.</p><p><strong>직후:</strong> 변경하면서 얻은 맥락을 활용하는 편이 다음에 다시 시작하는 것보다 유리할 때.</p><p><strong>나중:</strong> 당장 이익은 작지만 별도 시간에 작은 단위로 진행할 가치가 있을 때.</p><p><strong>하지 않기:</strong> 향후 변경이나 학습에서 기대하는 이익이 없을 때.</p><a class="source" href="{BASE}ch21.html">21장 근거 원문 ↗</a></section>
<section id="plan"><h2>6회차 학습 순서</h2><p>회차당 20~40분을 출발점으로 삼는 학습용 제안입니다. 코드를 바꾸는 실험은 별도 시간에 진행해도 됩니다.</p><ol class="sessions">{plan}</ol></section>
<section id="practices"><h2>33장 핵심 요지와 복습</h2><div class="progress-wrap"><div id="progress-label" aria-live="polite">이해한 항목 0 / 33</div><progress id="progress" max="33" value="0" aria-label="학습 진도"></progress></div>
<div class="toolbar"><label for="search">내용 검색</label><input id="search" type="search" placeholder="예: 결합, 주석, 시점"><div class="filters"><button data-filter="all" aria-pressed="true">전체</button>{filters}</div><div class="toolbar-bottom"><label><input type="checkbox" id="only-pending"> 아직 이해 표시하지 않은 항목만</label><span id="result-count" role="status"></span></div></div>
<p class="hint" id="save-status">진도와 메모는 현재 브라우저에 저장됩니다. 중요한 기록은 내보내세요. 브라우저나 파일 위치가 달라지면 기록이 보이지 않을 수 있습니다.</p><button class="export" id="export">진도와 메모 내보내기</button><p></p>{''.join(cards)}<p id="empty" hidden>검색 결과가 없습니다.</p></section>
<section id="examples" class="panel"><h2>코드로 확인하기</h2><p>{code_guide}</p><p><a href="examples.py">예제 파일</a> · <code>python3 examples.py</code></p><p class="small">정상 결과: <code>17 checks passed; incorrect guard clause detected</code></p><details><summary>예제 코드 펼치기</summary><pre class="code"><code>{esc((ROOT/'examples.py').read_text())}</code></pre></details><h3>직접 해보기</h3><p>기존 배송비 결과를 보존하며 구조만 정리하세요. 그다음 별도 단계에서 무료 배송 조건을 추가하고 경계 사례를 확인하세요. 두 단계의 차이를 설명해 보세요.</p></section>
<section class="panel"><h2>마지막 점검</h2><p>다음 변경은 무엇인가? 정리가 무엇을 쉽게 만들었는가? 결과와 부작용을 보존했는가? 지금 멈춰도 되는가?</p><a href="실험기록.md">작은 정리 실험 기록하기</a><p class="small">책의 비용 설명은 정밀한 예측식이 아닙니다. 검토 절차를 줄이는 제안도 팀의 안전성과 신뢰가 전제이며 실제 팀 정책을 대신하지 않습니다.</p></section>
<footer>전체 번역본이 아닌 간결한 한국어 요약과 독립 학습 자료입니다. 원문 코드와 그림을 복제하지 않았습니다. 원서 링크는 O’Reilly 접근 권한이 필요합니다.</footer></main></div>'''
(ROOT/'index.html').write_text(body+script)
(ROOT/'sources.json').write_text(json.dumps({'title':'Tidy First?', 'author':'Kent Beck', 'checked_on':'2026-10-05', 'scope':scope, 'chapters':[{'chapter':r['id'],'title_ko':r['title'],'url':BASE+r['page'],'verified':'rendered chapter text'} for r in rows]},ensure_ascii=False,indent=2)+'\n')
(ROOT/'README.md').write_text('''# Tidy First? 한국어 학습 자료

- [학습 화면](index.html): 33장 핵심 요지, 검색, 부별 선택, 답 가리기, 진도와 메모 저장·내보내기
- [한국어 노트](학습노트.md): 연속해서 읽기 위한 문서
- [실험 기록](실험기록.md): 복사해서 사용하는 적용 양식
- [코드 연습](examples.py): 직접 작성한 배송비 예제와 잘못된 보호 구문의 반례
- [출처](sources.json): 확인 범위와 장별 원문 링크

`index.html`을 브라우저로 열면 오프라인으로 공부할 수 있다. 원문 링크에는 O’Reilly 접근 권한이 필요하다. 설명은 한국어이며 코드 식별자는 영어다. 전체 번역본이 아니라 간결한 요약과 독립 연습 자료다.

`python3 examples.py`로 17개 검증과 잘못된 조기 반환의 탐지를 확인한다. 진도는 현재 브라우저 저장소에만 보관되므로 중요한 메모는 내보낸다.

내용은 `study.tsv`에서 수정한 뒤 `python3 build_notes.py`로 생성한다. 화면 스타일은 `style.css`, 학습 기능은 `controls.js`에서 수정한다. 다시 생성하면 학습 화면·노트·출처·README·실험 양식이 덮어써지므로 개인 실험 기록은 별도 파일로 저장한다.
''')
print(f'Built {len(rows)} chapter cards')
