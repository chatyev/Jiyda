// Приём статистики «Помочь улучшить Jiyda» (src/telemetry.rs): Cloudflare Worker + база D1.
//
// POST /v1/report — один отчёт за день от одной установки (JSON, как telemetry::Report).
// Отчёт проверяется строго: неизвестные поля и значения не по формату — 400, в базу
// не попадают. IP-адрес и заголовки запроса не сохраняются: в таблице только поля отчёта.
// Повторный отчёт той же установки за тот же день заменяет прежний.

const COUNTS = [
  'app_started', 'app_crashed', 'update_success', 'update_failed',
  'workspace_used', 'command_palette_used', 'import_completed', 'onboarding_completed',
];
const FIELDS = ['id', 'day', 'version', 'windows', 'counts', 'tabs', 'default_browser'];
const TABS = ['', '0', '1', '2-5', '6-10', '11-20', '21-50', '51+'];

function valid(r) {
  if (!r || typeof r !== 'object' || Array.isArray(r)) return false;
  if (Object.keys(r).some((k) => !FIELDS.includes(k)) || FIELDS.some((k) => !(k in r))) return false;
  if (!/^[0-9a-f]{32}$/.test(r.id)) return false;
  if (!/^\d{4}-\d{2}-\d{2}$/.test(r.day)) return false;
  if (typeof r.version !== 'string' || !/^\d{1,4}\.\d{1,4}\.\d{1,4}(-[a-z]{1,10}(\.\d{1,4})?)?$/.test(r.version)) return false;
  if (typeof r.windows !== 'string' || !/^(10\.0\.\d{1,6})?$/.test(r.windows)) return false;
  if (!TABS.includes(r.tabs) || typeof r.default_browser !== 'boolean') return false;
  const c = r.counts;
  if (!c || typeof c !== 'object' || Array.isArray(c)) return false;
  if (Object.keys(c).some((k) => !COUNTS.includes(k))) return false;
  return COUNTS.every((k) => Number.isInteger(c[k] ?? 0) && (c[k] ?? 0) >= 0 && (c[k] ?? 0) <= 100000);
}

// День отчёта — не из будущего и не старше двух недель (очередь в Jiyda — неделя).
function plausibleDay(day) {
  const t = Date.parse(day + 'T00:00:00Z');
  const now = Date.now();
  return Number.isFinite(t) && t <= now + 2 * 86400000 && t >= now - 14 * 86400000;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname !== '/v1/report') return new Response('Not found', { status: 404 });
    if (request.method !== 'POST') return new Response('Method not allowed', { status: 405, headers: { Allow: 'POST' } });
    const text = await request.text();
    if (text.length > 8192) return new Response('Too large', { status: 413 });
    let r;
    try { r = JSON.parse(text); } catch { return new Response('Bad JSON', { status: 400 }); }
    if (!valid(r) || !plausibleDay(r.day)) return new Response('Bad report', { status: 400 });
    const c = COUNTS.map((k) => r.counts[k] ?? 0);
    await env.DB.prepare(
      `INSERT OR REPLACE INTO reports (id, day, version, windows, tabs, default_browser, ${COUNTS.join(', ')})
       VALUES (?, ?, ?, ?, ?, ?, ${COUNTS.map(() => '?').join(', ')})`,
    ).bind(r.id, r.day, r.version, r.windows, r.tabs, r.default_browser ? 1 : 0, ...c).run();
    return new Response(null, { status: 204 });
  },
};
