// Поиск по странице. Вызывается из app.rs как (этот текст)(запрос, режим).
// Режимы: 'type' — запрос изменился, искать заново; 'next' / 'prev' — следующее /
// предыдущее совпадение; 'clear' — убрать подсветку.
// Подсветка — CSS Custom Highlight API: страница и выделение не меняются.
// Возвращает { n: число совпадений, i: номер текущего (с нуля) или -1 }.
(q, mode) => {
  const st = window.__jiydaFind || (window.__jiydaFind = { q: '', ranges: [], i: -1 });
  const hl = window.CSS && CSS.highlights;
  if (mode === 'clear' || !q) {
    if (hl) { hl.delete('jiyda-find'); hl.delete('jiyda-find-current'); }
    st.q = ''; st.ranges = []; st.i = -1;
    return { n: 0, i: -1 };
  }
  if (!document.getElementById('jiyda-find-style')) {
    const s = document.createElement('style');
    s.id = 'jiyda-find-style';
    s.textContent = '::highlight(jiyda-find){background:#fde68a;color:#111}'
      + '::highlight(jiyda-find-current){background:#f59e0b;color:#111}';
    (document.head || document.documentElement).appendChild(s);
  }

  if (mode === 'type' || q !== st.q) {
    const needle = q.toLowerCase(), ranges = [], LIMIT = 1000;
    const skip = /^(SCRIPT|STYLE|NOSCRIPT|TEXTAREA|TEMPLATE)$/;
    const walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT, {
      acceptNode(n) {
        const p = n.parentElement;
        if (!p || skip.test(p.tagName)) return NodeFilter.FILTER_REJECT;
        if (p.checkVisibility && !p.checkVisibility()) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      },
    });
    for (let n; ranges.length < LIMIT && (n = walker.nextNode());) {
      const t = n.data.toLowerCase();
      for (let i = t.indexOf(needle); i !== -1 && ranges.length < LIMIT; i = t.indexOf(needle, i + needle.length)) {
        const r = new Range();
        r.setStart(n, i);
        r.setEnd(n, i + needle.length);
        ranges.push(r);
      }
    }
    st.q = q;
    st.ranges = ranges;
    // Как в Chrome: начинаем с первого совпадения, которое не выше верха экрана.
    st.i = ranges.length ? Math.max(0, ranges.findIndex((r) => r.getBoundingClientRect().bottom >= 0)) : -1;
  } else if (st.ranges.length) {
    const n = st.ranges.length;
    st.i = (st.i + (mode === 'prev' ? n - 1 : 1)) % n;
  }

  if (hl) {
    hl.set('jiyda-find', new Highlight(...st.ranges));
    if (st.i >= 0) {
      const cur = new Highlight(st.ranges[st.i]);
      cur.priority = 1;
      hl.set('jiyda-find-current', cur);
    } else hl.delete('jiyda-find-current');
  }
  if (st.i >= 0) {
    const r = st.ranges[st.i], b = r.getBoundingClientRect();
    if (b.top < 0 || b.bottom > innerHeight) r.startContainer.parentElement.scrollIntoView({ block: 'center' });
  }
  return { n: st.ranges.length, i: st.i };
}
