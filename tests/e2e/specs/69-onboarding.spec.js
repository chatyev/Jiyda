// Знакомство с Jiyda (ui/js/onboarding.js). Само при первом запуске оно не появляется:
// профили E2E создаются с готовым config.json. Здесь его открывает команда палитры.

const settings = (t) => new Promise((resolve) => {
  S.on('data:settings', resolve);
  t.post({ type: 'get', what: 'settings' });
});
const nextBtn = (t) => t.$('.onboarding .ob-next');
const heading = (t) => (t.$('.onboarding h1') || {}).textContent;

E2E.test('onboarding: walk through import, theme and protection', async (t) => {
  t.cmd('onboarding');
  await t.waitFor(() => t.layerOpen('onboarding') && t.state().onboarding, 'знакомство открыто');
  t.eq(heading(t), S.t('Welcome to Jiyda'), 'приветствие');

  nextBtn(t).click();
  await t.waitFor(() => heading(t) === S.t('Bring your bookmarks'), 'шаг переноса (тестовые Chrome и Edge найдены)');
  const row = t.$$('.onboarding .ob-option').find((r) => r.textContent.includes('Google Chrome'));
  row.querySelector('button').click();
  // Шаг перерисовывается целиком — ищем строку заново.
  await t.waitFor(() => t.$$('.onboarding .ob-option').find((r) => r.textContent.includes('Google Chrome') && r.querySelector('.ob-done')), 'Chrome перенесён', 15000);

  nextBtn(t).click();
  await t.waitFor(() => heading(t) === S.t('Choose a look'), 'шаг темы');
  t.$$('.onboarding .theme-picker button')[1].click();
  await t.waitFor(() => t.state().theme === 'light', 'тема светлая');

  nextBtn(t).click();
  await t.waitFor(() => heading(t) === S.t('Private by default'), 'шаг защиты');
  t.$('.onboarding .switch').click();
  await t.waitFor(async () => (await settings(t)).shield === false, 'защита выключена');
  t.$('.onboarding .switch').click();
  await t.waitFor(async () => (await settings(t)).shield === true, 'и снова включена');

  // Браузер по умолчанию предлагается, только если Jiyda установлен на этой машине.
  nextBtn(t).click();
  await t.waitFor(() => [S.t('Open links in Jiyda?'), S.t('You’re all set')].includes(heading(t)), 'шаг после защиты');
  if (heading(t) === S.t('Open links in Jiyda?')) nextBtn(t).click();
  await t.waitFor(() => heading(t) === S.t('You’re all set'), 'последний шаг');
  nextBtn(t).click();
  await t.waitFor(() => !t.layerOpen('onboarding') && !t.state().onboarding, 'знакомство закрыто');
  t.ok(!t.state().importOffer, 'карточка переноса на New Page больше не нужна');

  // Убрать за собой: тема и перенесённые закладки.
  t.post({ type: 'setting', key: 'theme', value: 'system' });
  const items = await new Promise((resolve) => { S.on('data:bookmarks', (b) => resolve(b.items)); t.post({ type: 'get', what: 'bookmarks' }); });
  for (const n of items) t.post({ type: 'bookmark_remove', id: n.id });
});

E2E.test('onboarding: Skip closes it from the first step', async (t) => {
  t.cmd('onboarding');
  await t.waitFor(() => t.layerOpen('onboarding'), 'знакомство открыто');
  t.$('.onboarding .ob-skip').click();
  await t.waitFor(() => !t.layerOpen('onboarding') && !t.state().onboarding, 'закрыто кнопкой «Пропустить»');
});
