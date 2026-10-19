// Защита интерфейса (src/engine/webview2/mod.rs, src/app/ui_assets.rs): Content-Security-Policy
// не даёт ему ничего грузить из сети, а уйти он может только на свои страницы — сайт
// открывается обычной страницей Jiyda.

E2E.test('security: the UI cannot load anything from the network', async (t) => {
  const blocked = new Promise((resolve) => document.addEventListener('securitypolicyviolation', (e) => resolve(e.violatedDirective), { once: true }));
  const img = new Image();
  img.src = t.url('a.html') + '?csp';
  const directive = await Promise.race([blocked, t.sleep(3000).then(() => 'не заблокировано')]);
  t.eq(directive.split(' ')[0], 'img-src', 'внешняя картинка заблокирована');
  let fetched = false;
  try { await fetch(t.url('a.html')); fetched = true; } catch (_) { /* CSP: connect-src 'none' */ }
  t.ok(!fetched, 'fetch из интерфейса запрещён');
});

E2E.test('security: window.open from the UI becomes a Jiyda page, not a window', async (t) => {
  window.open(t.url('a.html'), '_blank');
  await t.waitFor(() => t.state().pages.some((p) => p.url === t.url('a.html')), 'сайт открылся страницей Jiyda');
  window.open('file:///C:/Windows/win.ini', '_blank');
  await t.sleep(500);
  t.eq(t.state().pages.length, 1, 'file: не открывается');
  t.ok(location.href.startsWith('https://jiyda.ui/'), 'интерфейс остался на месте');
});
