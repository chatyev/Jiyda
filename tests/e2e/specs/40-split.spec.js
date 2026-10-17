// Split View: две страницы рядом.

async function split(t) {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  t.post({ type: 'split', id: a.id });
  await t.waitFor(() => t.state().splits.length === 1, 'пара создана');
  return { a, b };
}

E2E.test('split: two pages side by side', async (t) => {
  const { a, b } = await split(t);
  const s = t.state().splits[0];
  t.eq([s.left, s.right].sort(), [a.id, b.id].sort(), 'в паре A и B');
  t.eq(t.state().visible.slice().sort(), [a.id, b.id].sort(), 'видны обе');
});

E2E.test('split: swap sides', async (t) => {
  await split(t);
  const before = t.state().splits[0];
  t.post({ type: 'split_swap' });
  await t.waitFor(() => t.state().splits[0].left === before.right && t.state().splits[0].right === before.left, 'поменялись местами');
});

E2E.test('split: ratio is kept within bounds', async (t) => {
  await split(t);
  t.post({ type: 'split_ratio', ratio: 0.3, done: true });
  await t.waitFor(() => Math.abs(t.state().splits[0].ratio - 0.3) < 0.01, 'доля 0.3');
  t.post({ type: 'split_ratio', ratio: 5, done: true });
  await t.sleep(300);
  const r = t.state().splits[0].ratio;
  t.ok(r > 0 && r < 1, `доля в пределах (0;1), получено ${r}`);
});

E2E.test('split: unsplit', async (t) => {
  await split(t);
  t.post({ type: 'unsplit' });
  await t.waitFor(() => t.state().splits.length === 0, 'пары нет');
  t.eq(t.state().visible.length, 1, 'видна одна страница');
});

E2E.test('split: closing one half breaks the pair', async (t) => {
  const { a, b } = await split(t);
  t.post({ type: 'close', id: b.id });
  await t.waitFor(() => t.state().splits.length === 0, 'пара распалась');
  t.eq(t.state().active, a.id, 'активна оставшаяся половина');
});

E2E.test('split: command opens a picker for the second page', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  t.cmd('split');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра в режиме выбора второй страницы');
  await t.waitFor(() => t.$$('#palette-list .row').some((r) => r.textContent.includes('Page A')), 'A среди вариантов');
  t.$$('#palette-list .row').find((r) => r.textContent.includes('Page A')).click();
  await t.waitFor(() => t.state().splits.length === 1, 'пара создана');
  const s = t.state().splits[0];
  t.eq([s.left, s.right].sort(), [a.id, b.id].sort(), 'в паре A и B');
});
