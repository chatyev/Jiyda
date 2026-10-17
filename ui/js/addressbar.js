// addressbar.js — адресная строка в центре панели, индикатор защиты и подсказки.
// Ctrl+L — изменить адрес; Enter — открыть здесь, Alt+Enter — новой страницей.
// Здесь же общий для адреса, New Page и палитры подбор подсказок (S.suggest).
import { S } from './core.js';

(() => {
  const box = S.$('#address');
  const view = S.$('#addr-view');
  const input = S.$('#addr-input');
  const privacy = S.$('#privacy');
  const reload = S.$('#addr-reload');
  const hint = S.$('#addr-hint');

  // ---------- Подсказки ----------
  let history = [];
  S.on('data:history', (h) => { history = h || []; });
  S.loadHistory = () => S.post({ type: 'get', what: 'history' });

  function looksLikeUrl(q) {
    return /^[a-z][a-z0-9+.-]*:\/\//i.test(q) || /^(about|edge|view-source):/i.test(q)
      || /^localhost(:\d+)?(\/|$)/i.test(q) || /^\d{1,3}(\.\d{1,3}){3}(:\d+)?(\/|$)/.test(q)
      || (!/\s/.test(q) && /^[^\s/.]+(\.[^\s/.]+)+(:\d+)?(\/.*)?$/.test(q) && /\.[a-zа-я-]{2,}(:\d+)?(\/|$)/i.test(q));
  }
  S.looksLikeUrl = looksLikeUrl;

  // Подсказки для запроса q: [{ kind: 'go'|'search'|'page'|'history', title, sub, url, id }].
  S.suggest = (q, { pages = 5, hist = 6, exclude = null } = {}) => {
    q = q.trim();
    const ql = q.toLowerCase();
    const out = [];
    if (!q) return out;
    if (looksLikeUrl(q)) out.push({ kind: 'go', title: q, sub: S.t('Open'), text: q, icon: 'globe' });
    else out.push({ kind: 'search', title: q, sub: S.t('Search'), text: q, icon: 'search' });
    const match = (s) => (s || '').toLowerCase().includes(ql);
    const open = S.state.pages.filter((p) => p.id !== exclude && (match(p.title) || match(p.url)));
    for (const p of open.slice(0, pages)) {
      const other = p.spaceId !== S.space().id ? S.state.spaces.find((s) => s.id === p.spaceId) : null;
      out.push({ kind: 'page', title: S.titleOf(p), sub: other ? `${p.domain} · ${other.name}` : p.domain, id: p.id, page: p, meta: S.t('Switch to page') });
    }
    const openUrls = new Set(S.state.pages.map((p) => p.url));
    const hits = history.filter((h) => !openUrls.has(h.url) && (match(h.title) || match(h.url)));
    hits.sort((a, b) => (S.host(b.url).startsWith(ql) - S.host(a.url).startsWith(ql)) || (b.visits - a.visits));
    for (const h of hits.slice(0, hist)) out.push({ kind: 'history', title: h.title || h.url, sub: S.host(h.url), text: h.url, url: h.url, favicon: h.favicon, meta: S.ago(h.ts) });
    return out;
  };

  // Строка подсказки (общая для адреса, New Page и палитры).
  S.suggestRow = (r, selected, onPick) => {
    const ico = r.page ? S.fav(r.page) : r.kind === 'history' ? S.fav({ url: r.url, title: r.title, favicon: r.favicon }) : S.h('span', { class: 'ico' }, S.icon(r.icon || 'globe', 'i16'));
    const el = S.h('button', { class: 'row' + (selected ? ' sel' : ''), attrs: { role: 'option', 'aria-selected': selected ? 'true' : 'false', tabindex: '-1' } },
      ico,
      S.h('span', { class: 'row-main' },
        S.h('span', { class: 'row-title', text: r.title }),
        r.sub ? S.h('span', { class: 'row-sub', text: r.sub }) : null),
      r.meta ? S.h('span', { class: 'row-meta', text: r.meta }) : null);
    el.addEventListener('mousedown', (e) => e.preventDefault());
    el.addEventListener('click', (e) => onPick(r, e));
    return el;
  };

  // Выполнить подсказку: перейти на страницу или открыть адрес.
  S.pickSuggestion = (r, mode) => {
    if (r.kind === 'page') S.post({ type: 'activate', id: r.id });
    else S.post({ type: 'navigate', text: r.text, mode });
  };

  // ---------- Отображение адреса ----------
  function render(st) {
    const act = S.activePage();
    const { host, rest } = act ? S.splitUrl(act.url) : { host: '', rest: '' };
    view.classList.toggle('placeholder', !act);
    S.$('.addr-host', view).textContent = act ? (host || S.titleOf(act)) : S.t('Search or enter a URL…');
    S.$('.addr-path', view).textContent = act ? rest : '';
    view.title = act ? act.url : '';
    box.classList.toggle('loading', !!(act && act.loading));
    reload.hidden = !act;
    S.fill(reload, S.icon(act && act.loading ? 'x' : 'rotate-cw', 'i16'));
    const rl = act && act.loading ? S.t('Stop') : S.withKey(S.t('Reload'), 'reload');
    reload.title = rl; reload.setAttribute('aria-label', rl);
    hint.textContent = S.keyOf('palette') || 'Ctrl K';
    hint.title = S.t('Command palette');
    // Индикатор защиты: щит; зелёная точка — соединение защищено и трекеры блокируются.
    const https = act && act.url.startsWith('https://');
    const web = act && /^https?:/.test(act.url);
    privacy.hidden = !act;
    privacy.classList.toggle('protected', !!(https && st.shield.on));
    privacy.classList.toggle('insecure', !!(web && !https));
    S.fill(privacy, S.icon(web && !https ? 'lock-open' : https && st.shield.on ? 'shield-check' : 'shield', 'i16'), S.h('span', { class: 'privacy-dot' }));
    const pl = !web ? S.t('Site information') : https ? S.t('Connection is secure — site information') : S.t('Connection is not secure — site information');
    privacy.title = pl; privacy.setAttribute('aria-label', pl);
    if (!editing) input.value = act ? act.url : '';
  }
  S.on('render', render);

  // ---------- Редактирование ----------
  let editing = false, items = [], sel = 0;
  const list = S.h('div', { class: 'pop suggest', hidden: true, attrs: { role: 'listbox', 'data-hit': '22' } });
  S.$('#popovers').append(list);

  function draw() {
    const q = input.value;
    const act = S.activePage();
    items = q.trim() && !(act && q === act.url) ? S.suggest(q, { exclude: S.state.active }) : [];
    S.fill(list, ...items.map((r, i) => S.suggestRow(r, i === sel, (rr, e) => pick(rr, e.altKey ? 'new' : null))));
    if (!items.length) { list.hidden = true; list.classList.remove('open'); return; }
    list.hidden = false;
    list.style.width = Math.max(box.offsetWidth, 420) + 'px';
    S.placeAt(list, box, { align: 'center', gap: 6 });
    list.classList.add('open');
  }

  function pick(r, mode) {
    const act = S.activePage();
    S.layers.close('address');
    S.pickSuggestion(r, mode || (act ? 'current' : 'new'));
  }

  S.layers.register('address', {
    pop: true,
    open() {
      editing = true;
      box.classList.add('editing');
      view.hidden = true;
      input.hidden = false;
      const act = S.activePage();
      input.value = act ? act.url : '';
      input.placeholder = S.t('Search or enter a URL…');
      sel = 0;
      S.loadHistory();
      requestAnimationFrame(() => { input.focus(); input.select(); });
      draw();
    },
    close() {
      editing = false;
      box.classList.remove('editing');
      view.hidden = false;
      input.hidden = true;
      list.hidden = true;
      list.classList.remove('open');
      const act = S.activePage();
      input.value = act ? act.url : '';
    },
  });

  input.addEventListener('input', () => { sel = 0; draw(); });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' && items.length) { e.preventDefault(); sel = Math.min(sel + 1, items.length - 1); draw(); }
    else if (e.key === 'ArrowUp' && items.length) { e.preventDefault(); sel = Math.max(sel - 1, 0); draw(); }
    else if (e.key === 'Enter') {
      e.preventDefault();
      const text = input.value.trim();
      if (!text) return;
      const mode = e.altKey ? 'new' : e.shiftKey ? 'split' : null;
      if (items[sel]) pick(items[sel], mode);
      else pick({ kind: 'go', text }, mode);
    }
  });
  S.on('data:history', () => { if (editing) draw(); });
  view.addEventListener('click', () => S.layers.open('address'));
  view.addEventListener('dblclick', (e) => e.stopPropagation());
  hint.addEventListener('click', () => S.layers.open('palette'));
  reload.addEventListener('click', () => { const act = S.activePage(); S.cmd(act && act.loading ? 'stop' : 'reload'); });
  privacy.addEventListener('click', () => S.layers.toggle('security', { anchor: privacy }));

  S.on('command:edit_url', () => {
    if (!S.activePage()) { S.emit('command:new_tab'); return; }
    S.layers.open('address');
  });
})();
