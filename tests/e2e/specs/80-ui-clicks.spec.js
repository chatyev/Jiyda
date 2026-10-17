// То, что пользователь делает мышью: кнопки верхней панели, меню ⋯, адресная строка,
// Undo в тосте, переключатель Spaces, режимы окна, запрос разрешения сайта.

const click = (el) => el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
const rowWith = (t, sel, text) => t.$$(sel).find((r) => r.textContent.includes(text));

E2E.test('ui: back / forward / reload buttons', async (t) => {
  await t.open('a.html');
  await t.evalPage('document.getElementById("to-b").click()');
  await t.waitFor(() => t.active().url === t.url('b.html') && t.active().title === 'Page B', 'на B');
  click(t.$('#back'));
  await t.waitFor(() => t.active().url === t.url('a.html'), 'кнопка «Назад»');
  click(t.$('#forward'));
  await t.waitFor(() => t.active().url === t.url('b.html'), 'кнопка «Вперёд»');
  await t.evalPage('window.__mark = 1');
  click(t.$('#addr-reload'));
  await t.waitFor(async () => (await t.evalPage('window.__mark === undefined')) === true, 'страница перезагрузилась');
});

E2E.test('ui: address bar — click, type, Enter navigates the current page', async (t) => {
  const a = await t.open('a.html');
  click(t.$('#addr-view'));
  await t.waitFor(() => !t.$('#addr-input').hidden && document.activeElement === t.$('#addr-input'), 'поле адреса в фокусе');
  t.type(t.$('#addr-input'), t.url('c.html'));
  t.key(t.$('#addr-input'), 'Enter');
  await t.waitFor(() => t.page(a.id).url === t.url('c.html') && t.page(a.id).title === 'Page C', 'та же страница ушла на C');
  t.eq(t.nowPages().length, 1, 'новая страница не появилась');
});

E2E.test('ui: address bar — Alt+Enter opens a new page', async (t) => {
  const a = await t.open('a.html');
  click(t.$('#addr-view'));
  await t.waitFor(() => !t.$('#addr-input').hidden, 'поле адреса');
  t.type(t.$('#addr-input'), t.url('b.html'));
  t.key(t.$('#addr-input'), 'Enter', { altKey: true });
  await t.waitFor(() => t.byUrl('b.html') && t.state().active !== a.id, 'B — новой страницей');
  t.eq(t.page(a.id).url, t.url('a.html'), 'A не изменилась');
});

E2E.test('ui: address bar — Escape cancels editing', async (t) => {
  const a = await t.open('a.html');
  click(t.$('#addr-view'));
  await t.waitFor(() => !t.$('#addr-input').hidden, 'поле адреса');
  t.type(t.$('#addr-input'), 'something else');
  t.key(t.$('#addr-input'), 'Escape');
  await t.waitFor(() => t.$('#addr-input').hidden, 'редактирование отменено');
  t.eq(t.page(a.id).url, t.url('a.html'), 'адрес не изменился');
});

E2E.test('ui: browser menu opens History', async (t) => {
  click(t.$('#menu-btn'));
  await t.waitFor(() => t.$('.menu') && rowWith(t, '.menu .menu-item', S.t('History')), 'меню с «Историей»');
  click(rowWith(t, '.menu .menu-item', S.t('History')));
  await t.waitFor(() => t.layerOpen('history'), 'история открыта из меню');
});

E2E.test('ui: browser menu — page actions disabled without a page', async (t) => {
  click(t.$('#menu-btn'));
  await t.waitFor(() => rowWith(t, '.menu .menu-item', S.t('Find on page')), 'меню');
  const find = rowWith(t, '.menu .menu-item', S.t('Find on page'));
  t.ok(find.disabled || find.getAttribute('aria-disabled') === 'true', '«Найти на странице» недоступно без страницы');
});

E2E.test('ui: Undo in the toast brings a closed page back', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  t.cmd('close'); // как Ctrl+W: закрыть активную с тостом «Страница закрыта · Отменить»
  await t.waitFor(() => !t.page(b.id), 'B закрыта');
  await t.waitFor(() => t.$('#toasts .toast button'), 'тост с кнопкой «Отменить»');
  click(t.$('#toasts .toast button'));
  await t.waitFor(() => t.byUrl('b.html'), 'B вернулась по «Отменить»');
});

E2E.test('ui: Spaces popover — create, rename by typing, switch by click', async (t) => {
  await t.open('a.html');
  click(t.$('#space-btn'));
  await t.waitFor(() => t.layerOpen('spaces'), 'переключатель');
  click(rowWith(t, '.pop.spaces .row', S.t('New Space')));
  await t.waitFor(() => t.state().spaces.length === 2 && t.$('.pop.spaces input.rename'), 'новое пространство и поле имени');
  const input = t.$('.pop.spaces input.rename');
  t.type(input, 'Учёба');
  t.key(input, 'Enter');
  await t.waitFor(() => t.state().spaces[1].name === 'Учёба', 'имя сохранено');
  await t.waitFor(() => rowWith(t, '.pop.spaces .row', 'Учёба'), 'строка с новым именем');
  // Переключиться обратно щелчком по основному.
  click(t.$$('.pop.spaces .row')[0]);
  await t.waitFor(() => t.state().activeSpace === 0, 'основное по щелчку');
});

E2E.test('ui: Focus Mode toggles', async (t) => {
  await t.open('a.html');
  t.cmd('focus');
  await t.waitFor(() => document.body.classList.contains('focus'), 'Focus Mode включён');
  t.cmd('focus');
  await t.waitFor(() => !document.body.classList.contains('focus'), 'выключен');
});

E2E.test('ui: full screen on and off', async (t) => {
  await t.open('a.html');
  t.cmd('fullscreen');
  await t.waitFor(() => t.state().fullscreen, 'во весь экран');
  t.cmd('fullscreen');
  await t.waitFor(() => !t.state().fullscreen, 'обычное окно');
});

E2E.test('ui: maximize and restore', async (t) => {
  const was = t.state().maximized;
  t.cmd('win_max');
  await t.waitFor(() => t.state().maximized !== was, 'развернуть/восстановить');
  t.cmd('win_max');
  await t.waitFor(() => t.state().maximized === was, 'обратно');
});

E2E.test('ui: Ask without API key shows setup', async (t) => {
  t.cmd('ask');
  await t.waitFor(() => t.layerOpen('ask'), 'Ask открыт');
  await t.waitFor(() => t.$('.ask-setup') || /API/i.test((t.$('.ask') || document.body).textContent), 'подсказка про ключ API');
});

// ----- Разрешения сайта -----

async function askGeo(t) {
  await t.open('geo.html');
  await t.evalPage('ask()');
  await t.waitFor(() => t.layerOpen('permission') && t.$('.pop.permission button'), 'запрос разрешения показан');
}

E2E.test('permissions: Block denies the site', async (t) => {
  await askGeo(t);
  click(rowWith(t, '.pop.permission button', S.t('Block')));
  await t.waitFor(async () => (await t.evalPage('window.__geo')) === 'denied', 'сайту отказано');
  await t.waitFor(() => !t.layerOpen('permission'), 'запрос закрыт');
});

E2E.test('permissions: Allow once is not remembered', async (t) => {
  await askGeo(t);
  click(rowWith(t, '.pop.permission button', S.t('Allow once')));
  await t.waitFor(async () => (await t.evalPage('window.__geo')) !== 'pending', 'ответ сайту');
  t.ok((await t.evalPage('window.__geo')) !== 'denied', 'не отказано');
  // Повторный запрос снова спрашивает: «один раз» не запоминается.
  await t.evalPage('window.__geo = "pending"; ask()');
  await t.waitFor(() => t.layerOpen('permission'), 'спрашивает снова');
  click(rowWith(t, '.pop.permission button', S.t('Block')));
});

E2E.test('permissions: Escape dismisses as denied once', async (t) => {
  await askGeo(t);
  S.layers.close('permission');
  await t.waitFor(async () => (await t.evalPage('window.__geo')) === 'denied', 'без ответа — отказ');
});
