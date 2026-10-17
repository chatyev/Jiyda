// ai.js — Ask Seyro: появляется только по запросу (кнопка ✦, Ctrl+Shift+A, палитра,
// выделенный текст, несколько страниц в Orbit). Постоянной панели ИИ нет.
// Работает с Claude по ключу Anthropic API пользователя (Settings → AI Assistant);
// текст страницы отправляется только когда пользователь сам нажал действие или спросил.
import { S } from './core.js';

(() => {
  const pop = S.h('div', { class: 'pop ask right', hidden: true, attrs: { role: 'dialog', 'data-hit': '22' } });
  S.$('#popovers').append(pop);
  const head = S.h('div', { class: 'ask-head' });
  const ctxRow = S.h('div', { class: 'ask-ctx' });
  const body = S.h('div', { class: 'ask-body', attrs: { 'aria-live': 'polite' } });
  const ta = S.h('textarea', { rows: 1 });
  const send = S.iconBtn('arrow-up', 'Send', () => submit(), 'sm');
  const inputRow = S.h('div', { class: 'ask-input' }, ta, send);
  const note = S.h('div', { class: 'ask-note' });
  pop.append(head, ctxRow, body, inputRow, note);

  // Контекст вопроса: { kind: 'page' | 'selection' | 'pages' | 'history' | 'titles' | 'none', pages, selection }
  let ctx = { kind: 'page', pages: [], selection: '' };
  let thread = []; // [{ who: 'me'|'ai'|'error'|'wait', text, req }]
  let nextReq = 1;

  const ACTIONS = [
    { id: 'summarize', icon: 'file-text', label: 'Summarize this page', needs: () => !!S.activePage(), run: () => ask(S.t('Summarize this page.'), { kind: 'page' }) },
    { id: 'explain', icon: 'book', label: 'Explain selected text', needs: () => !!ctx.selection, run: () => ask(S.t('Explain the selected text in simple terms.'), { kind: 'selection' }) },
    { id: 'translate', icon: 'translate', label: 'Translate', needs: () => !!ctx.selection || !!S.activePage(), run: () => {
      const into = S.cfg.lang === 'ru' ? 'Russian (if it is already in Russian, into English)' : 'English (if it is already in English, into Russian)';
      ask(ctx.selection ? `Translate the selected text into ${into}.` : `Translate the main content of this page into ${into}.`, { kind: ctx.selection ? 'selection' : 'page' });
    } },
    { id: 'compare', icon: 'compare', label: 'Compare pages', needs: () => comparePages().length >= 2, run: () => ask(S.t('Compare these pages: key similarities, differences and which is better for what.'), { kind: 'pages', pages: comparePages() }) },
    { id: 'organize', icon: 'grid', label: 'Organize pages', needs: () => S.pagesHere().filter((p) => p.state === 'now').length >= 3, run: () => ask(S.t('Suggest how to organize my open pages into a few groups, and which ones I can close or move to Later.'), { kind: 'titles' }) },
    { id: 'find', icon: 'history', label: 'Find something I saw', needs: () => true, run: () => { ctx = { ...ctx, kind: 'history' }; ta.value = ''; ta.placeholder = S.t('What do you remember about it?'); drawCtx(); ta.focus(); } },
    { id: 'research', icon: 'book', label: 'Research summary', needs: () => researchPages().length >= 1, run: () => ask(S.t('Write a short research summary across these pages: main findings, where sources agree or disagree, open questions.'), { kind: 'pages', pages: researchPages() }) },
  ];

  // Сравнивать: выбранные в Orbit, иначе обе половины Split View.
  function comparePages() {
    if (ctx.pages && ctx.pages.length >= 2) return ctx.pages;
    const sp = S.state.active && S.splitOf(S.state.active);
    return sp ? [sp.left, sp.right] : [];
  }
  // Исследование: выбранные страницы, иначе страницы группы текущей страницы.
  function researchPages() {
    if (ctx.pages && ctx.pages.length) return ctx.pages;
    const act = S.activePage();
    if (!act) return [];
    const g = S.state.groups.find((x) => x.ids.includes(act.id));
    return g ? g.ids.slice(0, 8) : [act.id];
  }

  const ready = () => S.state.prefs && S.state.prefs.aiKey;

  function ask(prompt, c) {
    if (!ready()) return;
    const kind = c.kind;
    const req = nextReq++;
    const pages = kind === 'pages' ? (c.pages || []) : [];
    const selection = kind === 'selection' || (kind === 'page' && ctx.selection) ? ctx.selection : '';
    thread.push({ who: 'me', text: prompt });
    thread.push({ who: 'wait', req });
    S.post({ type: 'ask', id: req, prompt, context: kind === 'selection' ? 'none' : kind, pages, selection });
    draw();
  }

  function submit() {
    const q = ta.value.trim();
    if (!q) return;
    ta.value = '';
    autosize();
    const kind = ctx.kind === 'history' ? 'history' : ctx.pages && ctx.pages.length ? 'pages' : ctx.selection ? 'selection' : S.activePage() ? 'page' : 'none';
    ask(q, { kind, pages: ctx.pages });
  }

  S.on('data:ai', (d) => {
    const i = thread.findIndex((m) => m.who === 'wait' && m.req === d.req);
    if (i < 0) return;
    if (d.error === 'no_key') thread.splice(i, 1, { who: 'error', text: S.t('Add your Anthropic API key in Settings → AI Assistant.') });
    else if (d.error) thread.splice(i, 1, { who: 'error', text: d.error });
    else thread.splice(i, 1, { who: 'ai', text: d.text });
    draw();
  });

  // Простая разметка ответа: абзацы, списки, **жирный**, `код`. Только текст, без HTML.
  function markdown(text) {
    const box = S.h('div', { class: 'ask-msg answer' });
    let listEl = null;
    const inline = (line) => {
      const frag = document.createDocumentFragment();
      const re = /(\*\*[^*]+\*\*|`[^`]+`)/g;
      let last = 0, m;
      while ((m = re.exec(line))) {
        if (m.index > last) frag.append(line.slice(last, m.index));
        const tok = m[0];
        frag.append(tok.startsWith('**') ? S.h('strong', { text: tok.slice(2, -2) }) : S.h('code', { text: tok.slice(1, -1) }));
        last = m.index + tok.length;
      }
      if (last < line.length) frag.append(line.slice(last));
      return frag;
    };
    for (const raw of text.split('\n')) {
      const line = raw.trimEnd();
      const bullet = /^\s*[-*•]\s+(.*)/.exec(line);
      const num = /^\s*\d+[.)]\s+(.*)/.exec(line);
      if (bullet || num) {
        const tag = bullet ? 'UL' : 'OL';
        if (!listEl || listEl.tagName !== tag) { listEl = S.h(tag.toLowerCase()); box.append(listEl); }
        listEl.append(S.h('li', {}, inline((bullet || num)[1])));
        continue;
      }
      listEl = null;
      if (!line.trim()) continue;
      const hd = /^#{1,6}\s+(.*)/.exec(line);
      box.append(hd ? S.h('p', {}, S.h('strong', { text: hd[1] })) : S.h('p', {}, inline(line)));
    }
    return box;
  }

  function drawCtx() {
    const act = S.activePage();
    const chips = [];
    if (ctx.kind === 'history') chips.push(S.h('span', { class: 'chip accent' }, S.icon('history', 'i12'), S.t('Your history')));
    else if (ctx.pages && ctx.pages.length) chips.push(S.h('span', { class: 'chip accent' }, S.icon('layers', 'i12'), S.t('{0} pages', ctx.pages.length)));
    else if (act) chips.push(S.h('span', { class: 'chip accent' }, S.fav(act), S.t('This page · {0}', act.domain || S.titleOf(act))));
    if (ctx.selection) chips.push(S.h('span', { class: 'chip', title: ctx.selection }, S.t('Selection: “{0}”', ctx.selection.length > 40 ? ctx.selection.slice(0, 40) + '…' : ctx.selection)));
    S.fill(ctxRow, ...chips);
    ctxRow.hidden = !ready() || !chips.length;
  }

  function draw() {
    S.fill(head, 
      S.h('h2', {}, S.icon('sparkle', 'i16'), 'Ask Seyro'),
      thread.length ? S.iconBtn('undo', S.t('New conversation'), () => { thread = []; draw(); }, 'sm') : null,
      S.iconBtn('x', S.t('Close'), () => S.layers.close('ask'), 'sm'));
    pop.setAttribute('aria-label', 'Ask Seyro');
    drawCtx();
    if (!ready()) {
      S.fill(body, S.h('div', { class: 'ask-setup' },
        S.h('p', { text: S.t('Ask Seyro uses Claude by Anthropic with your own API key. Page text is sent to Anthropic only when you ask a question or press an action.') }),
        S.h('button', { class: 'btn primary', text: S.t('Set up in Settings'), on: { click: () => S.layers.open('settings', { section: 'ai' }) } })));
      inputRow.hidden = true;
      note.hidden = true;
      return;
    }
    inputRow.hidden = false;
    note.hidden = false;
    note.textContent = S.t('Answers come from Claude and can be wrong. Page text is sent only when you ask.');
    ta.placeholder = ctx.kind === 'history' ? S.t('What do you remember about it?') : S.activePage() ? S.t('Ask about this page…') : S.t('Ask anything…');
    ta.setAttribute('aria-label', ta.placeholder);
    send.title = S.t('Send'); send.setAttribute('aria-label', S.t('Send'));
    const out = [];
    if (!thread.length) {
      out.push(S.h('div', { class: 'ask-actions' }, ...ACTIONS.map((a) => S.h('button', {
        class: 'ask-action', disabled: !a.needs(), on: { click: a.run },
      }, S.icon(a.icon), S.t(a.label)))));
    }
    for (const m of thread) {
      if (m.who === 'me') out.push(S.h('div', { class: 'ask-msg me', text: m.text }));
      else if (m.who === 'wait') out.push(S.h('div', { class: 'ask-thinking', attrs: { 'aria-label': S.t('Thinking…') } }, S.h('span'), S.h('span'), S.h('span')));
      else if (m.who === 'error') out.push(S.h('div', { class: 'ask-msg error', text: m.text }));
      else {
        const ans = markdown(m.text);
        out.push(ans, S.h('div', { style: { marginTop: '6px' } }, S.h('button', { class: 'link-btn', text: S.t('Copy'), on: { click: () => {
          const tx = S.h('textarea', { value: m.text }); document.body.append(tx); tx.select(); document.execCommand('copy'); tx.remove(); S.toast(S.t('Copied'));
        } } })));
      }
    }
    S.fill(body, ...out);
    body.scrollTop = body.scrollHeight;
  }

  function autosize() { ta.style.height = 'auto'; ta.style.height = Math.min(120, ta.scrollHeight) + 'px'; }
  ta.addEventListener('input', autosize);
  ta.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit(); } });

  function place() { S.placeAt(pop, S.$('#ask-btn').offsetParent ? S.$('#ask-btn') : S.$('#menu-btn'), { align: 'right', gap: 8 }); }

  S.layers.register('ask', {
    pop: true,
    open(a = {}) {
      ctx = { kind: a.pages ? 'pages' : 'page', pages: a.pages || [], selection: a.selection || '' };
      draw();
      place();
      S.show(pop);
      if (a.action) { const act = ACTIONS.find((x) => x.id === a.action); if (act && act.needs()) act.run(); }
      requestAnimationFrame(() => (ready() ? ta : pop).focus());
    },
    close() { S.hide(pop); },
  });
  pop.tabIndex = -1;
  S.on('render', () => { if (S.layers.isOpen('ask')) { drawCtx(); } });
  S.on('command:ask', () => S.layers.toggle('ask', {}));
  // Панель над выделенным текстом на странице: Explain / Translate / Ask Seyro.
  S.on('command:ask_selection', (d) => {
    const map = { explain: 'explain', translate: 'translate', ask: null };
    S.layers.open('ask', { selection: d.text, action: map[d.action] || undefined });
  });
})();
