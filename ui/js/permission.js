// permission.js — запрос разрешения сайта под адресной строкой:
// «meet.google.com wants to use your microphone» — Allow once · Always allow · Block.
// Главное действие — Allow once: разрешение только на этот раз, ничего не запоминается.
import { S } from './core.js';

(() => {
  const pop = S.h('div', { class: 'pop permission', hidden: true, attrs: { role: 'alertdialog', 'data-hit': '22' } });
  S.$('#popovers').append(pop);
  const queue = [];

  const WANTS = {
    camera: 'use your camera', microphone: 'use your microphone', location: 'know your location',
    notifications: 'show notifications', clipboard: 'see text and images you copy', sensors: 'use motion sensors',
    downloads: 'download multiple files', files: 'edit files on your device', fonts: 'use fonts installed on your device',
    windows: 'manage windows on your displays', midi: 'control your MIDI devices',
  };

  function answer(decision) {
    const q = queue.shift();
    if (!q) return;
    // Только решение: сайт и вид разрешения ядро помнит само.
    S.post({ type: 'permission_answer', id: q.id, decision });
    if (queue.length) draw(); else S.layers.close('permission');
  }

  function draw() {
    const q = queue[0];
    if (!q) return;
    const [ic] = S.permissionKind(q.kind);
    const site = S.host(q.origin);
    const once = S.h('button', { class: 'btn primary', text: S.t('Allow once'), on: { click: () => answer('once') } });
    S.fill(pop, 
      S.h('div', { class: 'q' }, S.icon(ic),
        S.h('div', {}, S.h('b', { text: site }), ' ', S.t('wants to ' + (WANTS[q.kind] || 'use a device feature')),
          queue.length > 1 ? S.h('div', { class: 'muted', text: S.t('{0} more requests', queue.length - 1) }) : null)),
      S.h('div', { class: 'acts' },
        S.h('button', { class: 'btn ghost', text: S.t('Block'), on: { click: () => answer('block') } }),
        S.h('button', { class: 'btn', text: S.t('Always allow'), on: { click: () => answer('always') } }),
        once));
    pop.setAttribute('aria-label', `${site} ${S.t('wants to ' + (WANTS[q.kind] || 'use a device feature'))}`);
    S.placeAt(pop, S.$('#address'), { gap: 8 });
    requestAnimationFrame(() => once.focus());
  }

  S.layers.register('permission', {
    pop: false,
    open() { draw(); S.show(pop); },
    close() {
      // Закрыли без ответа (Esc) — запросы отклоняются только на этот раз.
      while (queue.length) { const q = queue.shift(); S.post({ type: 'permission_answer', id: q.id, decision: 'dismiss' }); }
      S.hide(pop);
    },
  });

  S.on('command:permission', (q) => {
    // Запрос со страницы из другого Space отклоняется сразу (только на этот раз):
    // невидимая страница не должна получать доступ, пока пользователь её не видит.
    if (q.space !== S.state.activeSpace) { S.post({ type: 'permission_answer', id: q.id, decision: 'dismiss' }); return; }
    queue.push(q);
    if (S.layers.isOpen('permission')) draw(); else S.layers.open('permission');
  });
})();
