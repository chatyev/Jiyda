// Слои интерфейса: каждый открывается своей командой и закрывается (бывший ui-smoke.ps1).
const LAYERS = [
  ['palette', 'palette'], ['orbit', 'orbit'], ['spaces', 'spaces'], ['history', 'history'],
  ['closed', 'closed'], ['ask', 'ask'], ['settings', 'settings'], ['shortcuts', 'settings'], ['downloads', 'downloads'],
];

for (const [cmd, layer] of LAYERS) {
  E2E.test(`layers: ${cmd} opens and closes`, async (t) => {
    t.cmd(cmd);
    await t.waitFor(() => t.layerOpen(layer), `слой ${layer} открыт`);
    S.layers.close(layer);
    await t.waitFor(() => !t.layerOpen(layer), `слой ${layer} закрыт`);
  });
}

E2E.test('layers: one big layer at a time', async (t) => {
  t.cmd('palette');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра');
  t.cmd('history');
  await t.waitFor(() => t.layerOpen('history'), 'история');
  t.ok(!t.layerOpen('palette'), 'палитра закрылась, когда открылась история');
});

E2E.test('layers: Escape closes the top layer', async (t) => {
  t.cmd('palette');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра');
  t.key(document.activeElement || document.body, 'Escape');
  await t.waitFor(() => !t.layerOpen('palette'), 'палитра закрыта по Esc');
});

E2E.test('layers: theme command cycles system → light → dark → system', async (t) => {
  // Начинаем с «как в Windows» — от темы самой Windows (на CI светлая) тест не зависит.
  t.post({ type: 'setting', key: 'theme', value: 'system' });
  await t.waitFor(() => t.state().theme === 'system', 'как в Windows');
  for (const want of ['light', 'dark', 'system']) {
    t.cmd('theme');
    await t.waitFor(() => t.state().theme === want, `тема ${want}`);
    if (want !== 'system') t.eq(document.documentElement.dataset.theme, want, 'интерфейс перекрасился');
  }
});
