// New Page: карточки из настоящих данных вместо плиток сайтов (ui/js/home.js) и подсветка
// от цвета Space (styles.css, --glow): строка поиска, карточки, кнопка закладок.

const homeShown = (t) => t.$('.home') && !t.$('.home').hidden;
const card = (t, name) => t.$(`.home .hc-${name}`);
const rowsOf = (t, name) => [...(card(t, name) || document.createElement('i')).querySelectorAll('.hc-row')];
const css = (el, prop) => getComputedStyle(el).getPropertyValue(prop).trim();
const glow = () => css(document.documentElement, '--glow');

E2E.test('home: no site tiles; Spaces and Actions cards are always there', async (t) => {
  await t.waitFor(() => homeShown(t), 'New Page');
  t.eq(t.$$('.home .tile').length, 0, 'плиток сайтов нет');
  await t.waitFor(() => card(t, 'spaces') && card(t, 'actions'), 'карточки Spaces и Действия');
  t.ok(!card(t, 'continue'), 'без открытых страниц «Продолжить» не показывается');
  // Один Space — предлагается второй.
  t.ok(rowsOf(t, 'spaces').some((r) => r.textContent.includes(S.t('New Space'))), '«Новый Space»');
  t.eq(t.$('.home-cards').dataset.n, String(t.$$('.home-cards > .hc').length), 'data-n — число карточек');
  // Действия — только работающие: «Закладки» открывает панель закладок.
  rowsOf(t, 'actions')[0].click();
  await t.waitFor(() => t.layerOpen('bookmarks'), 'карточка открыла закладки');
});

E2E.test('home: Continue returns to the last page; Recent reopens a closed one', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  t.cmd('new_tab');
  await t.waitFor(() => homeShown(t) && card(t, 'continue'), '«Продолжить» на New Page');
  t.ok(card(t, 'continue').textContent.includes(b.title), 'последняя страница — B');
  t.ok(rowsOf(t, 'continue').some((r) => r.textContent.includes(S.t('All open pages'))), 'и «Все открытые страницы»');
  rowsOf(t, 'continue')[0].click();
  await t.waitFor(() => t.active() && t.active().id === b.id, 'вернулись к B');
  t.post({ type: 'close', id: a.id });
  t.post({ type: 'close', id: b.id });
  await t.waitFor(() => homeShown(t) && rowsOf(t, 'recent').some((r) => r.textContent.includes(a.title)), '«Недавние»: закрытая A');
  rowsOf(t, 'recent').find((r) => r.textContent.includes(a.title)).click();
  await t.waitFor(() => t.active() && t.active().url === t.url('a.html'), 'A открылась снова');
});

E2E.test('glow: search, cards and focus follow the Space color', async (t) => {
  await t.waitFor(() => homeShown(t), 'New Page');
  const search = t.$('.home-search');
  t.post({ type: 'space_color', index: 0, color: 'teal' });
  await t.waitFor(() => t.state().spaces[0].color === 'teal', 'бирюзовый');
  await t.sleep(900); // плавный переход цвета
  const teal = [glow(), getComputedStyle(search).borderTopColor, css(document.documentElement, '--focus-ring')];
  t.ok(teal[0], '--glow задан');
  t.post({ type: 'space_color', index: 0, color: 'orange' });
  await t.waitFor(() => t.state().spaces[0].color === 'orange', 'оранжевый');
  await t.waitFor(() => glow() !== teal[0], '--glow сменился без перезагрузки');
  await t.waitFor(() => getComputedStyle(search).borderTopColor !== teal[1], 'контур поиска перекрасился');
  t.ok(css(document.documentElement, '--focus-ring') !== teal[2], 'кольцо фокуса — того же цвета');
  t.ok(!/0, 229, 255|168, 85, 247/.test(getComputedStyle(search).boxShadow), 'никаких постоянных cyan/purple');
  // Акцент интерфейса (главные кнопки, переключатели, выделение) — тот же цвет Space.
  t.eq(css(document.documentElement, '--accent'), glow(), '--accent = --glow');
  const btn = S.h('button', { class: 'btn primary', text: 'x' });
  document.body.append(btn);
  const orange = getComputedStyle(btn).backgroundColor;
  t.post({ type: 'space_color', index: 0, color: 'teal' });
  await t.waitFor(() => getComputedStyle(btn).backgroundColor !== orange, 'главная кнопка перекрасилась вместе со Space');
  btn.remove();
  t.post({ type: 'space_color', index: 0, color: 'violet' });
});

E2E.test('glow: light theme uses a darker shade and softer glow', async (t) => {
  await t.waitFor(() => homeShown(t), 'New Page');
  const dark = S.state.theme;
  t.post({ type: 'setting', key: 'theme', value: 'dark' });
  await t.waitFor(() => document.documentElement.dataset.theme === 'dark', 'тёмная');
  await t.sleep(300);
  const onDark = glow();
  t.post({ type: 'setting', key: 'theme', value: 'light' });
  await t.waitFor(() => document.documentElement.dataset.theme === 'light', 'светлая');
  await t.waitFor(() => glow() !== onDark, 'подсветка сменилась вместе с темой');
  t.post({ type: 'setting', key: 'theme', value: dark || 'system' });
});
