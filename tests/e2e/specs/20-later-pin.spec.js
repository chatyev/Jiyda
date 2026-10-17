// Later (отложить) и закрепление.

E2E.test('later: moves page to Later and activates another', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  t.post({ type: 'later', id: b.id });
  await t.waitFor(() => t.page(b.id) && t.page(b.id).state === 'later', 'B в Later');
  t.eq(t.state().active, a.id, 'активна A');
  t.eq(t.nowPages().map((p) => p.id), [a.id], 'в Now только A');
});

E2E.test('later: undo returns page to Now', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  t.post({ type: 'later', id: b.id });
  await t.waitFor(() => t.page(b.id).state === 'later', 'B в Later');
  t.post({ type: 'later_undo', id: b.id, activate: true });
  await t.waitFor(() => t.page(b.id).state === 'now' && t.state().active === b.id, 'B снова в Now и активна');
});

E2E.test('later: activating a Later page brings it back', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  t.post({ type: 'later', id: b.id });
  await t.waitFor(() => t.page(b.id).state === 'later', 'B в Later');
  t.post({ type: 'activate', id: b.id });
  await t.waitFor(() => t.state().active === b.id && t.page(b.id).state === 'now', 'B активна и в Now');
});

E2E.test('pin: pinned page cannot go to Later', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  t.post({ type: 'pin', id: b.id, on: true });
  await t.waitFor(() => t.page(b.id).pinned, 'B закреплена');
  t.post({ type: 'later', id: b.id });
  await t.sleep(400);
  t.eq(t.page(b.id).state, 'now', 'закреплённая осталась в Now');
});

E2E.test('pin: closing a pinned page keeps it and resets its address', async (t) => {
  const a = await t.open('a.html');
  t.post({ type: 'pin', id: a.id, on: true });
  await t.waitFor(() => t.page(a.id).pinned, 'A закреплена');
  await t.evalPage('document.getElementById("to-b").click()');
  await t.waitFor(() => t.page(a.id).url === t.url('b.html'), 'закреплённая ушла на B');
  t.post({ type: 'close', id: a.id });
  await t.waitFor(() => t.page(a.id) && t.page(a.id).url === t.url('a.html'), 'вернулась к адресу закрепления');
  t.ok(t.page(a.id).pinned, 'осталась закреплённой');
});

E2E.test('pin: unpin', async (t) => {
  const a = await t.open('a.html');
  t.post({ type: 'pin', id: a.id, on: true });
  await t.waitFor(() => t.page(a.id).pinned, 'закреплена');
  t.post({ type: 'pin', id: a.id, on: false });
  await t.waitFor(() => !t.page(a.id).pinned, 'откреплена');
});
