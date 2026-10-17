// Orbit: фильтр по категориям. Цвет пространства: градиент с переливом на всё окно.
// (Сообщения пользователя: «в orbit нет фильтрации вкладок по категориям»,
//  «цветовые схемы меняются только в одной области экрана, а всё окно должно меняться
//   и быть градиентом с переливаниями».)

// Пять страниц в двух категориях: localhost — «Разработка» (так в правилах orbit.rs),
// docs.* — «Документация». Домен .test не существует: в сеть тест не ходит, а страница
// появляется в состоянии сразу (загрузку не ждём).
const DOCS = ['https://docs.jiyda.test/guide', 'https://docs.jiyda.test/api'];
async function fivePages(t) {
  for (const p of ['a.html', 'b.html', 'c.html']) await t.open(p);
  for (const url of DOCS) {
    t.post({ type: 'navigate', text: url, mode: 'new' });
    await t.waitFor(() => t.state().pages.some((p) => p.url.startsWith(url)), `страница ${url}`);
  }
  await t.waitFor(() => t.state().groups.length >= 2, `две группы в состоянии (${JSON.stringify(t.state().groups.map((g) => [g.key, g.ids.length]))})`);
}

// Видимые в Orbit страницы теста: A/B/C и обе страницы документации.
// Фильтр готов: Orbit открыт и показывает кнопки (скрытый слой хранит старые элементы,
// поэтому одного наличия кнопок мало).
const filterReady = (t) => t.layerOpen('orbit') && !t.$('.orbit-filter').hidden && t.$$('.orbit-filter button').length >= 3;

const visibleTitles = (t) => t.$$('.orbit [role="option"]').map((o) => o.textContent)
  .filter((s) => /Page [ABC]|docs\.jiyda\.test/.test(s));

E2E.test('orbit filter: category chips with counts', async (t) => {
  await fivePages(t);
  t.cmd('orbit');
  await t.waitFor(() => t.layerOpen('orbit'), 'Orbit');
  await t.waitFor(() => filterReady(t), 'фильтры: «Все» и две категории');
  const labels = t.$$('.orbit-filter button').map((b) => b.textContent.replace(/\s+/g, ' ').trim());
  t.ok(labels.some((l) => l.includes(S.t('Documentation')) && l.includes('2')), `«Документация 2» среди ${JSON.stringify(labels)}`);
  t.ok(labels.some((l) => l.includes(S.t('Development')) && l.includes('3')), `«Разработка 3» среди ${JSON.stringify(labels)}`);
  t.eq(t.$('.orbit-filter [aria-pressed="true"]').textContent.includes(S.t('All')), true, 'сначала выбрано «Все»');
});

E2E.test('orbit filter: choosing a category shows only its pages', async (t) => {
  await fivePages(t);
  t.cmd('orbit');
  await t.waitFor(() => filterReady(t), 'фильтры');
  t.$$('.orbit-filter button').find((b) => b.textContent.includes(S.t('Documentation'))).click();
  await t.waitFor(() => {
    const v = visibleTitles(t);
    return v.length === 2 && v.every((s) => /docs\.jiyda\.test/.test(s));
  }, `только страницы документации (сейчас ${JSON.stringify(visibleTitles(t))})`);
  t.$$('.orbit-filter button').find((b) => b.textContent.includes(S.t('All'))).click();
  await t.waitFor(() => visibleTitles(t).length === 5, 'снова все пять');
});

E2E.test('orbit filter: works together with search and resets on reopen', async (t) => {
  await fivePages(t);
  t.cmd('orbit');
  await t.waitFor(() => filterReady(t), 'фильтры');
  t.$$('.orbit-filter button').find((b) => b.textContent.includes(S.t('Development'))).click();
  t.type(t.$('.orbit input'), 'Page B');
  await t.waitFor(() => { const v = visibleTitles(t); return v.length === 1 && v[0].includes('Page B'); }, 'фильтр + поиск → только B');
  S.layers.close('orbit');
  await t.waitFor(() => !t.layerOpen('orbit'), 'закрыт');
  t.cmd('orbit');
  await t.waitFor(() => t.layerOpen('orbit') && t.$('.orbit-filter [aria-pressed="true"]'), 'открыт снова');
  t.ok(t.$('.orbit-filter [aria-pressed="true"]').textContent.includes(S.t('All')), 'фильтр сброшен на «Все»');
});

// ----- Цвет пространства на всё окно -----
// Фон окна (body) прозрачен — под ним страница. «Всё окно» — то, что видно вокруг страницы:
// верхняя панель, New Page, строки Split View, затемнение под слоями.

const bg = (el, pseudo) => getComputedStyle(el, pseudo || null).backgroundImage;
const homeShown = (t) => t.$('.home') && !t.$('.home').hidden;
const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches || document.documentElement.dataset.motion === 'reduced';

E2E.test('space color: top bar and New Page are gradients', async (t) => {
  await t.waitFor(() => homeShown(t), 'New Page');
  t.ok(/gradient/.test(bg(t.$('#chrome'))), `верхняя панель — градиент (сейчас ${bg(t.$('#chrome'))})`);
  t.ok(/gradient/.test(bg(t.$('.home'))), 'New Page — градиент');
  await t.open('a.html'); // со страницей цвет пространства всё равно виден — в верхней панели
  t.ok(/gradient/.test(bg(t.$('#chrome'))), 'со страницей верхняя панель — градиент');
});

E2E.test('space color: changing the color repaints top bar and New Page', async (t) => {
  await t.waitFor(() => homeShown(t), 'New Page');
  t.post({ type: 'space_color', index: 0, color: 'green' });
  await t.waitFor(() => t.state().spaces[0].color === 'green', 'зелёный');
  await t.sleep(1000); // плавный переход цвета
  const green = [bg(t.$('#chrome')), bg(t.$('.home'))];
  t.post({ type: 'space_color', index: 0, color: 'orange' });
  await t.waitFor(() => t.state().spaces[0].color === 'orange', 'оранжевый');
  await t.waitFor(() => bg(t.$('#chrome')) !== green[0], 'верхняя панель перекрасилась');
  await t.waitFor(() => bg(t.$('.home')) !== green[1], 'New Page перекрасилась');
  t.post({ type: 'space_color', index: 0, color: 'violet' });
});

E2E.test('space color: gradients shimmer unless motion is reduced', async (t) => {
  await t.waitFor(() => homeShown(t), 'New Page');
  const top = getComputedStyle(t.$('#chrome'), '::before').animationName;
  const home = getComputedStyle(t.$('.home')).animationName;
  if (reducedMotion()) {
    t.eq([top, home], ['none', 'none'], 'при «меньше движения» перелива нет');
  } else {
    t.ok(top && top !== 'none', `перелив верхней панели (animation-name: ${top})`);
    t.ok(home && home !== 'none', `перелив New Page (animation-name: ${home})`);
  }
});

E2E.test('space color: switching spaces changes the window color', async (t) => {
  await t.open('a.html');
  t.post({ type: 'space_color', index: 0, color: 'blue' });
  t.post({ type: 'space_new' });
  await t.waitFor(() => t.state().spaces.length === 2 && t.state().activeSpace === 1, 'второе пространство');
  S.layers.closeAll();
  t.post({ type: 'space_color', index: 1, color: 'rose' });
  await t.waitFor(() => t.state().spaces[1].color === 'rose', 'розовое');
  await t.sleep(1000);
  const second = bg(t.$('#chrome'));
  t.post({ type: 'space', index: 0 });
  await t.waitFor(() => t.state().activeSpace === 0, 'основное');
  await t.waitFor(() => bg(t.$('#chrome')) !== second, 'верхняя панель — в цвете основного');
  t.post({ type: 'space_color', index: 0, color: 'violet' });
});

E2E.test('space color: Split View headers are tinted', async (t) => {
  const a = await t.open('a.html');
  await t.open('b.html');
  t.post({ type: 'split', id: a.id });
  await t.waitFor(() => t.$('.pane-head'), 'строки над половинами');
  const head = getComputedStyle(t.$('.pane-head'));
  t.ok(/gradient/.test(head.backgroundImage) || head.backgroundColor !== getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(),
    `строка Split View в цвете пространства (${head.backgroundImage} / ${head.backgroundColor})`);
});