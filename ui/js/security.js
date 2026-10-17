// security.js — панель «Защита» по щелчку на значок слева в адресной строке.
// Показывает только то, что Jiyda действительно делает: соединение (HTTPS),
// заблокированные на странице трекеры и реклама (shield.rs), изоляцию куки по Space,
// запомненные разрешения сайта — и выключатель защиты для этого сайта.
import { S } from './core.js';

(() => {
  const pop = S.h('div', { class: 'pop security', hidden: true, attrs: { role: 'dialog', 'data-hit': '22' } });
  S.$('#popovers').append(pop);
  let info = null;

  const KIND = {
    camera: ['camera', 'Camera'], microphone: ['mic', 'Microphone'], location: ['map-pin', 'Location'],
    notifications: ['bell', 'Notifications'], clipboard: ['copy', 'Clipboard'], sensors: ['eye', 'Motion sensors'],
    downloads: ['download', 'Automatic downloads'], files: ['file', 'File editing'], fonts: ['file-text', 'Local fonts'],
    windows: ['columns', 'Window management'], midi: ['sliders', 'MIDI devices'],
  };
  S.permissionKind = (k) => KIND[k] || ['info', k];

  function draw() {
    const d = info;
    if (!d) { S.fill(pop, S.h('div', { class: 'empty', text: S.t('No page open') })); return; }
    const protectedHere = d.shield && d.siteShield;
    const blocked = d.trackers + d.ads;
    const status = !d.web
      ? S.h('div', { class: 'sec-status' }, S.icon('info', 'i20'), S.h('div', {}, S.t('This is a local or internal page')))
      : d.https
        ? S.h('div', { class: 'sec-status' }, S.icon('lock', 'i20'), S.h('div', {}, S.t('Connection is secure'),
          S.h('small', { text: protectedHere ? (blocked ? S.t('Jiyda blocked {0} trackers and ads on this page', blocked) : S.t('Jiyda is protecting you on this site')) : S.t('Protection is off for this site') })))
        : S.h('div', { class: 'sec-status warn' }, S.icon('lock-open', 'i20'), S.h('div', {}, S.t('Connection is not secure'),
          S.h('small', { text: S.t('Don’t enter passwords or payment details on this site') })));
    const perms = Object.entries(d.permissions || {});
    const rowEl = (iconName, k, v, cls = '') => S.h('div', { class: 'sec-row' }, S.icon(iconName), S.h('span', { class: 'k', text: k }), S.h('span', { class: 'v ' + cls, text: v }));
    const toggle = S.h('button', {
      class: 'switch', attrs: { role: 'switch', 'aria-checked': d.siteShield ? 'true' : 'false', 'aria-label': S.t('Protection on this site'), disabled: !d.shield || !d.web },
      on: { click: () => S.post({ type: 'site_shield', host: d.site, on: !d.siteShield }) },
    });
    const rows = [
      d.web ? rowEl(d.https ? 'lock' : 'lock-open', S.t('SSL certificate'), d.https ? S.t('Valid') : S.t('No encryption'), d.https ? 'ok' : '') : null,
      d.web ? rowEl('eye', S.t('Trackers blocked'), protectedHere ? String(d.trackers) : '—') : null,
      d.web ? rowEl('ban', S.t('Ads blocked'), protectedHere ? String(d.ads) : '—') : null,
      rowEl('cookie', S.t('Cookies'), S.t('Isolated in “{0}”', d.space)),
      d.web ? rowEl('key', S.t('Site permissions'), d.allowed ? S.t('{0} allowed', d.allowed) : perms.length ? S.t('Blocked') : S.t('None asked')) : null,
      ...perms.map(([kind, dec]) => {
        const [ic, label] = S.permissionKind(kind);
        return S.h('div', { class: 'sec-row' }, S.icon(ic), S.h('span', { class: 'k', text: S.t(label) }),
          S.h('span', { class: 'v ' + (dec === 'allow' ? 'ok' : ''), text: dec === 'allow' ? S.t('Allowed') : S.t('Blocked') }),
          S.h('button', { class: 'link-btn', text: S.t('Reset'), on: { click: () => S.post({ type: 'permission_revoke', origin: d.origin, kind }) } }));
      }),
      d.web ? S.h('div', { class: 'sec-row' }, S.icon('shield'), S.h('span', { class: 'k', text: S.t('Block trackers and ads here') }), toggle) : null,
    ];
    S.fill(pop, 
      S.h('div', { class: 'sec-head' }, S.fav({ url: d.url, favicon: d.favicon, title: d.site }, 'lg'),
        S.h('div', { class: 'ellipsis' }, S.h('div', { class: 'site ellipsis', text: d.site || d.url }), S.h('div', { class: 'space-tag', text: S.t('in Space “{0}”', d.space) }))),
      status,
      S.h('div', { class: 'sec-rows' }, ...rows),
      S.h('div', { class: 'sec-foot' },
        S.h('button', { class: 'btn', text: S.t('Site settings'), on: { click: () => { S.layers.open('settings', { section: 'privacy', origin: d.origin }); } } }),
        S.h('span', { style: { flex: 1 } }),
        !d.shield ? S.h('button', { class: 'btn primary', text: S.t('Turn protection on'), on: { click: () => S.post({ type: 'setting', key: 'shield', value: true }) } }) : null));
    pop.setAttribute('aria-label', S.t('Site information'));
  }

  S.on('data:security', (d) => { info = d; if (S.layers.isOpen('security')) { draw(); place(); } });
  let anchor = null;
  const place = () => anchor && S.placeAt(pop, anchor, { gap: 8, dx: -8 });

  S.layers.register('security', {
    pop: true,
    open(a = {}) {
      anchor = a.anchor || S.$('#privacy');
      info = null;
      S.post({ type: 'get', what: 'security' });
      draw();
      place();
      S.show(pop);
      pop.tabIndex = -1;
      pop.focus();
    },
    close() { S.hide(pop); },
  });
  S.on('render', () => { if (S.layers.isOpen('security')) S.post({ type: 'get', what: 'security' }); });
})();
