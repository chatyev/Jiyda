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

E2E.test('layers: theme toggles and comes back', async (t) => {
  const before = document.documentElement.dataset.theme;
  t.cmd('theme');
  await t.waitFor(() => document.documentElement.dataset.theme && document.documentElement.dataset.theme !== before, 'тема сменилась');
  t.cmd('theme');
  await t.waitFor(() => t.state().theme, 'тема в состоянии');
});
