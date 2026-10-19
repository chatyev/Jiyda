// onboarding.js — знакомство с Jiyda при самом первом запуске (ядро: state.onboarding).
// Шаги: приветствие → перенос из Chrome/Edge (если найдены) → тема → защита →
// браузер по умолчанию (если Jiyda установлен) → главные клавиши. Всё можно пропустить;
// повторить — командой палитры «Знакомство с Jiyda».
import { S } from './core.js';

(() => {
  const el = S.h('div', { class: 'onboarding', hidden: true, attrs: { role: 'dialog', 'aria-modal': 'true' } });
  const card = S.h('div', { class: 'ob-card' });
  el.append(card);
  document.body.append(el);

  let step = 0, cfg = null, imp = null, importing = null;
  const imported = {};
  S.on('data:settings', (c) => { cfg = c; if (S.layers.isOpen('onboarding')) draw(); });
  S.on('data:import', (d) => {
    imp = d;
    if (d && d.done && importing) { imported[importing] = d.done; importing = null; }
    if (S.layers.isOpen('onboarding')) draw();
  });

  const set = (key, value) => S.post({ type: 'setting', key, value });
  const toggle = (key, value, label) => S.h('button', {
    class: 'switch', attrs: { role: 'switch', 'aria-checked': value ? 'true' : 'false', 'aria-label': label },
    on: { click: () => { cfg[key] = !value; set(key, !value); draw(); } },
  });
  const option = (title, desc, control) => S.h('div', { class: 'ob-option' },
    S.h('div', { class: 'text' }, S.h('b', { text: title }), desc ? S.h('span', { text: desc }) : null), control);

  // ---------- Шаги ----------
  function welcome() {
    return {
      body: [
        S.h('img', { class: 'ob-logo', src: S.cfg.logo, alt: '' }),
        S.h('h1', { text: S.t('Welcome to Jiyda') }),
        S.h('p', { class: 'ob-lead', text: S.t('A calmer way to browse. A few quick choices and you’re ready — it takes a minute.') }),
      ],
      next: S.t('Get started'),
    };
  }

  function importStep() {
    const rows = imp.sources.map((s) => {
      const done = imported[s.id];
      const busy = importing === s.id;
      const label = s.profile ? `${s.name} · ${s.profile}` : s.name;
      return option(label,
        done ? S.t('Imported: {0} bookmarks and {1} history entries.', done.bookmarks, done.history) : null,
        done ? S.h('span', { class: 'ob-done' }, S.icon('check', 'i16'), S.t('Done'))
          : S.h('button', { class: 'btn', text: busy ? S.t('Importing…') : S.t('Import'), disabled: !!importing,
            on: { click: () => { importing = s.id; S.post({ type: 'import', source: s.id, bookmarks: true, history: true }); draw(); } } }));
    });
    return {
      body: [
        S.h('h1', { text: S.t('Bring your bookmarks') }),
        S.h('p', { class: 'ob-lead', text: S.t('Move bookmarks and history from the browser you used before. Nothing is removed there, and passwords stay in that browser.') }),
        S.h('div', { class: 'ob-list' }, ...rows),
      ],
      next: Object.keys(imported).length ? S.t('Continue') : S.t('Not now'),
      busy: !!importing,
    };
  }

  function themeStep() {
    const themes = [['system', S.t('System')], ['light', S.t('Light')], ['dark', S.t('Dark')]];
    return {
      body: [
        S.h('h1', { text: S.t('Choose a look') }),
        S.h('p', { class: 'ob-lead', text: S.t('“System” follows Windows dark and light mode. You can change it any time in Settings.') }),
        S.h('div', { class: 'theme-picker ob-themes', attrs: { role: 'radiogroup', 'aria-label': S.t('Theme') } }, ...themes.map(([k, l]) => S.h('button', {
          attrs: { role: 'radio', 'aria-checked': S.state.theme === k ? 'true' : 'false' }, on: { click: () => set('theme', k) },
        }, S.h('span', { class: 'sw ' + k }), l))),
      ],
      next: S.t('Continue'),
    };
  }

  function protectionStep() {
    return {
      body: [
        S.h('h1', { text: S.t('Private by default') }),
        S.h('p', { class: 'ob-lead', text: S.t('Jiyda doesn’t collect your history, searches or page contents. There are no ads and no analytics.') }),
        S.h('div', { class: 'ob-list' },
          option(S.t('Block trackers and ads'), S.t('Known trackers and ad networks don’t load. You can turn it off for a single site.'), toggle('shield', cfg.shield, S.t('Block trackers and ads'))),
          option(S.t('Check for updates automatically'), S.t('Jiyda asks GitHub for a new version twice a day and installs it only when you agree.'), toggle('check_updates', cfg.check_updates, S.t('Check for updates automatically'))),
          cfg.telemetry_info && cfg.telemetry_info.available
            ? option(S.t('Help improve Jiyda'), S.t('Send anonymous usage statistics once a day. Never addresses, searches or anything you type.'), toggle('telemetry', cfg.telemetry, S.t('Help improve Jiyda')))
            : null),
      ],
      next: S.t('Continue'),
    };
  }

  function defaultStep() {
    return {
      body: [
        S.h('h1', { text: S.t('Open links in Jiyda?') }),
        S.h('p', { class: 'ob-lead', text: S.t('Make Jiyda your default browser so links from other apps open here. Windows asks you to confirm it in its Settings.') }),
        S.h('div', { class: 'ob-actions' }, S.h('button', { class: 'btn', text: S.t('Open Windows Settings'), on: { click: () => S.cmd('default_browser') } })),
      ],
      next: S.t('Continue'),
    };
  }

  function doneStep() {
    const tip = (cmd, fallback, text) => S.h('div', { class: 'ob-tip' }, S.h('kbd', { text: S.keyOf(cmd) || fallback }), S.h('span', { text }));
    return {
      body: [
        S.h('h1', { text: S.t('You’re all set') }),
        S.h('p', { class: 'ob-lead', text: S.t('Jiyda stays out of the way. These keys bring everything back:') }),
        S.h('div', { class: 'ob-tips' },
          tip('palette', 'Ctrl K', S.t('Search pages, history, bookmarks and commands')),
          tip('orbit', 'Ctrl Space', S.t('Orbit: all your open pages, grouped')),
          tip('new_tab', 'Ctrl T', S.t('New page')),
          tip('spaces', 'Ctrl Shift Space', S.t('Switch Space'))),
      ],
      next: S.t('Start browsing'),
    };
  }

  function steps() {
    if (!cfg) return [welcome];
    return [
      welcome,
      imp && imp.sources && imp.sources.length ? importStep : null,
      themeStep,
      protectionStep,
      cfg.default_browser ? defaultStep : null,
      doneStep,
    ].filter(Boolean);
  }

  function draw() {
    const list = steps();
    step = Math.min(step, list.length - 1);
    const s = list[step]();
    const last = step === list.length - 1;
    el.setAttribute('aria-label', S.t('Welcome to Jiyda'));
    S.fill(card,
      S.h('button', { class: 'btn ghost ob-skip', text: S.t('Skip'), on: { click: finish } }),
      S.h('div', { class: 'ob-body' }, ...s.body),
      S.h('div', { class: 'ob-foot' },
        S.h('div', { class: 'ob-dots', attrs: { 'aria-hidden': 'true' } }, ...list.map((_, i) => S.h('span', { class: i === step ? 'on' : '' }))),
        step > 0 ? S.h('button', { class: 'btn ghost', text: S.t('Back'), on: { click: () => { step--; draw(); } } }) : null,
        S.h('button', { class: 'btn primary ob-next', text: s.next, disabled: !!s.busy, on: { click: () => (last ? finish() : (step++, draw())) } })));
    requestAnimationFrame(() => { const b = S.$('.ob-next', card); if (b && !card.contains(document.activeElement)) b.focus(); });
  }

  function finish() {
    S.layers.close('onboarding');
  }

  S.layers.register('onboarding', {
    full: true,
    open() {
      step = 0;
      el.hidden = false;
      S.post({ type: 'get', what: 'settings' });
      S.post({ type: 'get', what: 'import' });
      draw();
      requestAnimationFrame(() => el.classList.add('open'));
    },
    close() {
      S.hide(el);
      if (S.state.onboarding) S.cmd('onboarding_done');
    },
  });

  // Ядро просит показать знакомство (первый запуск или команда палитры).
  S.on('render', (st) => {
    if (st.onboarding && !S.layers.isOpen('onboarding')) S.layers.open('onboarding');
    else if (S.layers.isOpen('onboarding')) draw();
  });
})();
