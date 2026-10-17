// menu.js — контекстные меню: компактные, с клавиатурой (↑↓ Enter Esc, → подменю).
// S.menu.open(где, пункты). Пункт: { label, icon, kbd, danger, disabled, action, submenu },
// 'sep' — разделитель, { swatches: [{ color, current, action, label }] } — цвета.
import { S } from './core.js';

S.menu = (() => {
  let stack = []; // открытые меню: корневое и подменю
  let opener = null;

  function close(all = true) {
    const drop = all ? stack : stack.slice(-1);
    for (const m of drop) m.el.remove();
    stack = all ? [] : stack.slice(0, -1);
    if (all && opener !== null) {
      S.holdKeyboard(false);
      const o = opener;
      opener = null;
      if (o && o.isConnected) o.focus({ preventScroll: true });
    }
    S.region.update();
  }

  function build(items, level) {
    const el = S.h('div', { class: 'menu', attrs: { role: 'menu', 'data-hit': '22' } });
    const entries = [];
    for (const it of items) {
      if (!it) continue;
      if (it === 'sep') { el.append(S.h('div', { class: 'menu-sep', attrs: { role: 'separator' } })); continue; }
      if (it.swatches) {
        const row = S.h('div', { class: 'menu-swatches' });
        for (const sw of it.swatches) {
          row.append(S.h('button', {
            class: sw.current ? 'cur' : '', style: { background: sw.color }, title: sw.label,
            attrs: { 'aria-label': sw.label, role: 'menuitemradio', 'aria-checked': sw.current ? 'true' : 'false' },
            on: { click: () => { close(); sw.action(); } },
          }));
        }
        el.append(row);
        continue;
      }
      const b = S.h('button', {
        class: 'menu-item' + (it.danger ? ' danger' : ''),
        attrs: { role: 'menuitem', 'aria-disabled': it.disabled ? 'true' : null, 'aria-haspopup': it.submenu ? 'menu' : null, tabindex: '-1' },
      },
      it.icon ? S.icon(it.icon) : S.h('span', { class: 'i' }),
      S.h('span', { class: 'label', text: it.label }),
      it.kbd ? S.h('kbd', { text: it.kbd }) : null,
      it.submenu ? S.icon('chevron-right', 'i12') : null);
      const entry = { el: b, it };
      b.addEventListener('mouseenter', () => select(level, entries.indexOf(entry), false));
      b.addEventListener('click', () => activate(level, entry));
      entries.push(entry);
      el.append(b);
    }
    return { el, entries, sel: -1 };
  }

  function select(level, i, focus = true) {
    const m = stack[level];
    if (!m) return;
    m.sel = i;
    m.entries.forEach((e, j) => e.el.classList.toggle('sel', j === i));
    if (focus && m.entries[i]) m.entries[i].el.focus({ preventScroll: true });
    // Наведение на другой пункт закрывает открытое подменю.
    while (stack.length > level + 1) close(false);
    const it = m.entries[i] && m.entries[i].it;
    if (it && it.submenu && !focus) openSub(level, m.entries[i]);
  }

  function openSub(level, entry) {
    if (stack.length > level + 1) return;
    const sub = build(entry.it.submenu, level + 1);
    document.body.append(sub.el);
    stack.push(sub);
    const r = entry.el.getBoundingClientRect();
    S.placeAt(sub.el, { left: r.right + 2, right: r.right, top: r.top - 4, bottom: r.top - 4, width: 0, height: 0 }, { gap: 0 });
    requestAnimationFrame(() => sub.el.classList.add('open'));
    S.region.update();
  }

  function activate(level, entry) {
    if (entry.it.disabled) return;
    if (entry.it.submenu) { openSub(level, entry); select(level + 1, 0); return; }
    close();
    entry.it.action && entry.it.action();
  }

  function open(where, items) {
    close();
    opener = document.activeElement;
    const m = build(items, 0);
    document.body.append(m.el);
    stack = [m];
    let anchor;
    if (where instanceof Element) anchor = where.getBoundingClientRect();
    else if (where && 'clientX' in where) anchor = { left: where.clientX, right: where.clientX, top: where.clientY, bottom: where.clientY, width: 0, height: 0 };
    else anchor = where;
    S.placeAt(m.el, anchor, { gap: where instanceof Element ? 4 : 0, align: where instanceof Element && anchor.left > innerWidth / 2 ? 'right' : 'left' });
    requestAnimationFrame(() => m.el.classList.add('open'));
    S.holdKeyboard(true);
    select(0, 0);
    S.region.update();
  }

  addEventListener('keydown', (e) => {
    if (!stack.length) return;
    const level = stack.length - 1, m = stack[level];
    const n = m.entries.length;
    const enabled = (i) => m.entries[i] && !m.entries[i].it.disabled;
    const step = (d) => { let i = m.sel; for (let k = 0; k < n; k++) { i = (i + d + n) % n; if (enabled(i)) return i; } return m.sel; };
    if (e.key === 'ArrowDown') select(level, step(1));
    else if (e.key === 'ArrowUp') select(level, step(-1));
    else if (e.key === 'Home') select(level, 0);
    else if (e.key === 'End') select(level, n - 1);
    else if (e.key === 'ArrowRight' && m.entries[m.sel] && m.entries[m.sel].it.submenu) activate(level, m.entries[m.sel]);
    else if (e.key === 'ArrowLeft' && level > 0) { close(false); select(level - 1, stack[level - 1].sel); }
    else if (e.key === 'Enter' || e.key === ' ') { if (m.entries[m.sel]) activate(level, m.entries[m.sel]); }
    else if (e.key === 'Escape') { if (level > 0) close(false); else close(); }
    else if (e.key === 'Tab') close();
    else return;
    e.preventDefault();
    e.stopImmediatePropagation();
  }, true);
  addEventListener('mousedown', (e) => { if (stack.length && !e.target.closest('.menu')) close(); }, true);
  S.on('command:close_pops', () => { if (stack.length) close(); });
  addEventListener('resize', () => { if (stack.length) close(); });

  return { open, close: () => close(), isOpen: () => stack.length > 0 };
})();
