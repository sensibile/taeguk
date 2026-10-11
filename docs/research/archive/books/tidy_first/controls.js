
const key = 'tidy-first-study-ko-v1';
let state = {done: {}, notes: {}};
let storageAvailable = true;
try { const raw = localStorage.getItem(key); if(raw) { const parsed = JSON.parse(raw); state = {done: parsed.done || {}, notes: parsed.notes || {}}; } } catch (_) { storageAvailable = false; }
const cards = [...document.querySelectorAll('.practice')];
const progress = document.getElementById('progress');
const search = document.getElementById('search');
const pending = document.getElementById('only-pending');
let part = 'all';
function save() { try { localStorage.setItem(key, JSON.stringify(state)); } catch (_) { storageAvailable = false; document.getElementById('save-status').textContent = '이 브라우저에서 자동 저장을 사용할 수 없습니다. 창을 닫기 전에 진도와 메모를 내보내세요.'; } }
function update() {
 const count = cards.filter(c => state.done[c.id.slice(1)] === true).length;
 progress.value = count;
 document.getElementById('progress-label').textContent = `이해한 항목 ${count} / ${cards.length}`;
 const q = search.value.trim().toLocaleLowerCase();
 let visible = 0;
 cards.forEach(c => {
   const id = c.id.slice(1);
   const text = c.textContent + ' ' + (state.notes[id] || '');
   const show = (part === 'all' || c.dataset.part === part) && (!pending.checked || state.done[id] !== true) && text.toLocaleLowerCase().includes(q);
   c.hidden = !show;
   if(show) visible++;
 });
 document.getElementById('result-count').textContent = `${visible}개 표시`;
 document.getElementById('empty').hidden = visible !== 0;
}
document.querySelectorAll('[data-done]').forEach(el => {
 el.checked = state.done[el.dataset.done] === true;
 el.addEventListener('change', () => {state.done[el.dataset.done] = el.checked; save(); update();});
});
document.querySelectorAll('[data-note]').forEach(el => {
 el.value = typeof state.notes[el.dataset.note] === 'string' ? state.notes[el.dataset.note] : '';
 el.addEventListener('input', () => {state.notes[el.dataset.note] = el.value; save();});
});
function selectPart(value) {
 part = value;
 document.querySelectorAll('[data-filter]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.filter === value)));
 update();
}
document.querySelectorAll('[data-filter]').forEach(b => b.addEventListener('click', () => selectPart(b.dataset.filter)));
document.querySelectorAll('[data-jump]').forEach(a => a.addEventListener('click', () => {search.value = ''; pending.checked = false; selectPart('all');}));
search.addEventListener('input', update);
pending.addEventListener('change', update);
document.getElementById('export').addEventListener('click', () => {
 const lines = ['# tidy-first 학습 기록', '', `저장 시각: ${new Date().toLocaleString('ko-KR')}`, ''];
 cards.forEach(c => {
   const id = c.id.slice(1);
   lines.push(`## ${id}장 ${c.querySelector('h3').textContent}`, '', `이해 표시: ${state.done[id] === true ? '완료' : '미완료'}`, '', state.notes[id] || '작성한 메모 없음', '');
 });
 const url = URL.createObjectURL(new Blob([lines.join('\n')], {type:'text/markdown;charset=utf-8'}));
 const a = document.createElement('a'); a.href = url; a.download = 'tidy-first-학습기록.md'; a.click();
 setTimeout(() => URL.revokeObjectURL(url), 1000);
});
window.addEventListener('beforeprint', () => document.querySelectorAll('.question details').forEach(d => {d.dataset.wasOpen = String(d.open);d.open = true;}));
window.addEventListener('afterprint', () => document.querySelectorAll('.question details').forEach(d => {d.open = d.dataset.wasOpen === 'true';}));
if(!storageAvailable) document.getElementById('save-status').textContent = '저장된 기록을 불러오지 못했거나 로컬 저장소를 사용할 수 없습니다. 중요한 기록은 파일로 내보내세요.';
update();
