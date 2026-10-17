// harness.js — мини-фреймворк E2E, выполняется в интерфейсе Jiyda (WebView интерфейса).
// run.ps1 загружает его, затем файлы specs/*.spec.js и вызывает E2E.run(filter).
// Результаты уходят в журнал ядра строками «E2E PASS|FAIL|SKIP …» и «E2E DONE …».
//
// Тест:  E2E.test('pages: close activates neighbour', async (t) => { … });
// Перед каждым тестом — t.reset(): ни страниц, ни слоёв, одно пространство «Основное».
(() => {
  const tests = [];
  const log = (msg) => S.post({ type: 'log', msg });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  // Ответы страниц на e2e_eval (см. UiMsg::E2eEval в ядре).
  let seq = 0;
  const waiting = new Map();
  S.on('data:e2e', (d) => {
    const w = waiting.get(d.req);
    if (!w) return;
    waiting.delete(d.req);
    if (d.error) w.reject(new Error(d.error)); else w.resolve(d.value);
  });

  class AssertionError extends Error {}
  const show = (v) => { try { return JSON.stringify(v); } catch (_) { return String(v); } };

  const t = {
    base: window.E2E_BASE || 'http://localhost:8765/',
    sleep,
    state: () => S.state,
    url: (path = '') => t.base + path,
    post: (msg) => S.post(msg),
    cmd: (k) => S.cmd(k),
    /// Страницы текущего пространства (как их видит интерфейс).
    pages: () => S.pagesHere(),
    nowPages: () => S.pagesHere().filter((p) => p.state === 'now'),
    page: (id) => S.state.pages.find((p) => p.id === id),
    active: () => S.state.pages.find((p) => p.id === S.state.active) || null,
    byUrl: (path) => S.state.pages.find((p) => p.url === t.url(path)),
    layerOpen: (name) => S.layers.isOpen(name),
    $: (sel) => document.querySelector(sel),
    $$: (sel) => [...document.querySelectorAll(sel)],

    ok(cond, msg = 'ожидалось истинное значение') { if (!cond) throw new AssertionError(msg); },
    eq(actual, expected, msg = '') {
      if (show(actual) !== show(expected)) throw new AssertionError(`${msg ? msg + ': ' : ''}ожидалось ${show(expected)}, получено ${show(actual)}`);
    },

    /// Ждать, пока cond() станет истинным (опрос каждые 50 мс).
    async waitFor(cond, what = 'условие', ms = 10000) {
      const end = Date.now() + ms;
      let last;
      for (;;) {
        // Условие может быть и асинхронным (запрос данных у ядра) — ждём его результат.
        try { last = await cond(); if (last) return last; } catch (_) { /* ещё рано */ }
        if (Date.now() > end) throw new AssertionError(`не дождались: ${what} (${ms} мс)`);
        await sleep(50);
      }
    },

    /// Открыть адрес так, как это делает адресная строка; дождаться загрузки. → страница.
    async open(path, mode = 'new') {
      const url = /^[a-z]+:/.test(path) ? path : t.url(path);
      t.post({ type: 'navigate', text: url, mode });
      return t.waitFor(() => {
        const p = S.state.pages.find((x) => x.url === url && x.id === S.state.active);
        return p && !p.loading && p.title && p.title !== url ? p : null;
      }, `загрузка ${url}`, 15000);
    },

    /// Выполнить код в странице (0 — активная) и вернуть результат.
    evalPage(code, id = 0) {
      const req = ++seq;
      return new Promise((resolve, reject) => {
        waiting.set(req, { resolve, reject });
        S.post({ type: 'e2e_eval', req, id, code });
        setTimeout(() => { if (waiting.delete(req)) reject(new AssertionError('страница не ответила за 5 с')); }, 5000);
      });
    },

    /// Ввести текст в поле интерфейса так, будто его напечатали.
    type(el, text) {
      el.focus();
      el.value = text;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    key(el, key, opts = {}) {
      el.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...opts }));
    },

    /// Чистое состояние между тестами.
    async reset() {
      S.layers.closeAll();
      if (S.state.activeSpace !== 0) {
        t.post({ type: 'space', index: 0 });
        await t.waitFor(() => S.state.activeSpace === 0, 'возврат в «Основное»');
      }
      for (let i = S.state.spaces.length - 1; i > 0; i--) t.post({ type: 'space_delete', index: i });
      await t.waitFor(() => S.state.spaces.length === 1, 'удаление лишних пространств');
      if (S.state.splits.length) t.post({ type: 'unsplit' });
      for (const p of S.state.pages) {
        if (p.pinned) t.post({ type: 'pin', id: p.id, on: false });
        t.post({ type: 'close', id: p.id });
      }
      await t.waitFor(() => S.state.pages.length === 0, 'закрытие всех страниц');
      t.post({ type: 'clear_closed' });
      await t.waitFor(() => !(S.state.closed || []).length, 'очистка закрытых');
      // Запомненные разрешения сайтов («Разрешать всегда», «Запретить»): иначе следующий
      // тест не увидит запроса.
      const perms = await new Promise((resolve) => {
        const done = (s) => resolve((s && s.permissions) || {});
        S.on('data:settings', done);
        t.post({ type: 'get', what: 'settings' });
        setTimeout(() => resolve({}), 2000);
      });
      for (const [origin, kinds] of Object.entries(perms)) {
        for (const kind of Object.keys(kinds || {})) t.post({ type: 'permission_revoke', origin, kind });
      }
      S.layers.closeAll();
      await sleep(100);
    },
  };

  window.E2E = {
    t,
    test(name, fn, opts = {}) { tests.push({ name, fn, timeout: opts.timeout || 30000, skip: opts.skip }); },
    /// `reset: false` — для проверок после перезапуска: состояние нужно как есть.
    async run(filter = '', { reset = true } = {}) {
      let pass = 0, fail = 0, skip = 0;
      const re = filter ? new RegExp(filter, 'i') : null;
      for (const tc of tests) {
        if ((re && !re.test(tc.name)) || tc.skip) { skip++; log(`E2E SKIP ${tc.name}${tc.skip ? ' ⟶ ' + tc.skip : ''}`); continue; }
        const t0 = Date.now();
        try {
          if (reset) await t.reset();
          await Promise.race([
            tc.fn(t),
            sleep(tc.timeout).then(() => { throw new AssertionError(`тест не уложился в ${tc.timeout} мс`); }),
          ]);
          pass++;
          log(`E2E PASS ${tc.name} (${Date.now() - t0} мс)`);
        } catch (e) {
          fail++;
          const where = e instanceof AssertionError ? '' : ` [${(e && e.stack || '').split('\n')[1] || ''}]`;
          log(`E2E FAIL ${tc.name} ⟶ ${e && e.message || e}${where}`);
        }
      }
      log(`E2E DONE pass=${pass} fail=${fail} skip=${skip}`);
    },
  };
})();
