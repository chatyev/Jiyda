// «Помочь улучшить Jiyda» (src/telemetry.rs, Настройки → Приватность). По умолчанию выключено;
// отчёт — только счётчики. Адрес приёма в E2E — несуществующий домен (run.ps1).

const settingsData = (t) => new Promise((resolve) => {
  S.on('data:settings', resolve);
  t.post({ type: 'get', what: 'settings' });
});
const report = async (t) => (await settingsData(t)).telemetry_info.preview;

E2E.test('telemetry: off by default, turned on only by the user', async (t) => {
  const c = await settingsData(t);
  t.ok(c.telemetry_info.available, 'в этой сборке есть адрес приёма');
  t.ok(!c.telemetry, 'по умолчанию выключено');
  t.eq(c.telemetry_info.preview, null, 'выключено — и отчёта нет');

  S.layers.open('settings', { section: 'privacy' });
  const row = await t.waitFor(() => t.$$('.settings .setting').find((r) => r.textContent.includes(S.t('Send anonymous usage statistics'))), 'строка статистики');
  row.querySelector('.switch').click();
  await t.waitFor(async () => (await settingsData(t)).telemetry, 'включено');
  const show = await t.waitFor(() => t.$$('.settings .setting').find((r) => r.textContent.includes(S.t('What is sent')))?.querySelector('button'), '«Что отправляется»');
  show.click();
  const pre = await t.waitFor(() => t.$('.settings .telemetry-report'), 'отчёт показан');
  const shown = JSON.parse(pre.textContent);
  t.eq(Object.keys(shown).sort(), ['counts', 'day', 'default_browser', 'id', 'tabs', 'version', 'windows'], 'только эти поля');
  t.ok(!/https?:|localhost/.test(pre.textContent), 'ни одного адреса');
  t.eq(shown.counts.app_started, 1, 'запуск посчитан при включении');

  // Палитра считается, адреса страниц — нет.
  await t.open('a.html');
  S.layers.closeAll();
  S.layers.open('palette');
  await t.waitFor(async () => (await report(t)).counts.command_palette_used === 1, 'палитра посчитана');
  t.ok(!JSON.stringify(await report(t)).includes('a.html'), 'открытая страница в отчёт не попала');

  // Выключили — номер установки и счётчики забыты.
  const id = (await report(t)).id;
  t.post({ type: 'setting', key: 'telemetry', value: false });
  await t.waitFor(async () => (await report(t)) === null, 'выключено — отчёта нет');
  t.post({ type: 'setting', key: 'telemetry', value: true });
  const fresh = await t.waitFor(async () => report(t), 'снова включено');
  t.ok(fresh.id !== id && fresh.counts.command_palette_used === 0, 'новый номер, счётчики с нуля');
  t.post({ type: 'setting', key: 'telemetry', value: false });
});
