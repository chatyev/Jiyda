// spaces.js — переключатель пространств (Spaces): Work 12, Personal 8, … + New Space.
// У каждого Space свои страницы, куки и логины. ↑↓ Enter — выбрать, F2 — переименовать.
import { S } from './core.js';

(() => {
  const pop = S.h('div', { class: 'pop spaces', hidden: true, attrs: { role: 'menu', 'aria-label': 'Spaces', 'data-hit': '22' } });
  S.$('#popovers').append(pop);
  let sel = 0, renaming = -1, confirming = -1, anchor = null;
  // Сохранить имя, которое сейчас редактируется (при закрытии панели).
  let commitRename = () => {};

  function colorMenu(i) {
    const s = S.state.spaces[i];
    return { swatches: Object.entries(S.SPACE_COLORS).map(([key, color]) => ({
      color, current: key === s.color, label: S.t('Color'), action: () => S.post({ type: 'space_color', index: i, color: key }),
    })) };
  }

  function spaceMenu(i, at) {
    const s = S.state.spaces[i];
    S.menu.open(at, [
      { label: S.t('Rename'), icon: 'sliders', kbd: 'F2', action: () => { renaming = i; draw(); } },
      colorMenu(i),
      s.main ? null : 'sep',
      s.main ? null : { label: S.t('Delete Space…'), icon: 'trash', danger: true, action: () => { confirming = i; draw(); } },
    ]);
  }

  function draw() {
    const st = S.state;
    const rows = st.spaces.map((s, i) => {
      if (confirming === i) {
        return S.h('div', { class: 'row sel', attrs: { role: 'alert' } },
          S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: S.t('Delete “{0}” with its pages, cookies and logins?', s.name) })),
          S.h('button', { class: 'btn danger', text: S.t('Delete'), on: { click: () => { confirming = -1; S.post({ type: 'space_delete', index: i }); } } }),
          S.h('button', { class: 'btn ghost', text: S.t('Cancel'), on: { click: () => { confirming = -1; draw(); } } }));
      }
      if (renaming === i) {
        const inp = S.h('input', { class: 'rename', value: s.name, attrs: { 'aria-label': S.t('Space name'), maxlength: '40' } });
        const done = (save) => {
          if (renaming !== i) return;
          renaming = -1;
          if (save && inp.value.trim()) S.post({ type: 'space_rename', index: i, name: inp.value });
          draw();
        };
        inp.addEventListener('keydown', (e) => {
          if (e.key === 'Enter') { e.preventDefault(); done(true); }
          else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); done(false); }
          e.stopPropagation();
        });
        // Щелчок в другое место окна — сохранить. Уход в другую программу (Alt+Tab) — нет:
        // поле остаётся, как при переименовании в Проводнике.
        inp.addEventListener('blur', () => { if (document.hasFocus()) done(true); });
        commitRename = () => done(true);
        requestAnimationFrame(() => { inp.focus(); inp.select(); });
        return S.h('div', { class: 'row sel' }, S.h('span', { class: 'dot', style: { background: S.colorOf(s.color) } }), inp);
      }
      const cur = i === st.activeSpace;
      const more = S.iconBtn('more', S.t('Space options'), (e) => { e.stopPropagation(); spaceMenu(i, e.currentTarget); }, 'sm');
      const el = S.h('button', {
        class: 'row' + (i === sel ? ' sel' : ''),
        title: i < 9 ? `${s.name} (Ctrl Shift ${i + 1})` : s.name,
        attrs: { role: 'menuitemradio', 'aria-checked': cur ? 'true' : 'false', tabindex: '-1' },
        on: {
          click: () => { S.layers.close('spaces'); if (!cur) S.post({ type: 'space', index: i }); },
          contextmenu: (e) => { e.preventDefault(); spaceMenu(i, e); },
          mousemove: () => { if (sel !== i) { sel = i; mark(); } },
        },
      },
      S.h('span', { class: 'dot', style: { background: S.colorOf(s.color) } }),
      S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: s.name })),
      S.h('span', { class: 'row-meta hide-on-hover', text: s.pages || '' }),
      S.h('span', { class: 'row-acts' }, more),
      S.h('span', { class: 'check', style: { width: '16px' } }, cur ? S.icon('check', 'i16') : null));
      return el;
    });
    const add = S.h('button', { class: 'row', attrs: { role: 'menuitem' }, on: { click: () => S.post({ type: 'space_new' }) } },
      S.h('span', { class: 'ico' }, S.icon('plus', 'i16')),
      S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: S.t('New Space') })));
    S.fill(pop, ...rows, S.h('div', { class: 'pop-foot' }, add));
    if (anchor) S.placeAt(pop, anchor, { gap: 6 });
  }
  function mark() { S.$$('.row', pop).forEach((r, i) => r.classList.toggle('sel', i === sel && renaming < 0 && confirming < 0)); }

  S.layers.register('spaces', {
    pop: true,
    open(arg = {}) {
      anchor = arg.anchor || S.$('#space-btn');
      sel = S.state.activeSpace;
      renaming = arg.rename ?? -1;
      confirming = -1;
      S.$('#space-btn').setAttribute('aria-expanded', 'true');
      draw();
      S.show(pop);
      pop.focus();
    },
    close() {
      S.$('#space-btn').setAttribute('aria-expanded', 'false');
      commitRename();
      commitRename = () => {};
      renaming = confirming = -1;
      S.hide(pop);
    },
  });
  pop.tabIndex = -1;
  pop.addEventListener('keydown', (e) => {
    if (renaming >= 0 || confirming >= 0) return;
    const n = S.state.spaces.length;
    if (e.key === 'ArrowDown') sel = (sel + 1) % n;
    else if (e.key === 'ArrowUp') sel = (sel - 1 + n) % n;
    else if (e.key === 'Enter') { S.layers.close('spaces'); if (sel !== S.state.activeSpace) S.post({ type: 'space', index: sel }); return; }
    else if (e.key === 'F2') { renaming = sel; draw(); return; }
    else if (e.key === 'Delete' && !S.state.spaces[sel].main) { confirming = sel; draw(); return; }
    else if (/^[1-9]$/.test(e.key) && +e.key <= n) { S.layers.close('spaces'); S.post({ type: 'space', index: +e.key - 1 }); return; }
    else return;
    e.preventDefault();
    mark();
  });
  // Пока имя редактируется, не перерисовываем: поле ввода не должно пропадать из-под курсора.
  S.on('render', () => { if (S.layers.isOpen('spaces') && renaming < 0) draw(); });
  S.on('command:spaces', () => S.layers.toggle('spaces', {}));
  S.on('command:space_created', (idx) => S.layers.open('spaces', { rename: idx }));
})();
