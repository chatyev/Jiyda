-- Метрики из плана (раздел 37): запускать в Cloudflare → D1 → jiyda-telemetry → Console
-- или `wrangler d1 execute jiyda-telemetry --remote --file metrics.sql`.
-- Считаются только те, кто сам включил статистику, — это выборка, а не все пользователи.

-- Активные установки по дням (DAU) за последние 30 дней.
SELECT day, COUNT(*) AS dau
FROM reports WHERE day >= date('now', '-30 days')
GROUP BY day ORDER BY day;

-- MAU: установки, активные хотя бы день за последние 30 дней.
SELECT COUNT(DISTINCT id) AS mau FROM reports WHERE day >= date('now', '-30 days');

-- Удержание D1 / D7 / D30: из тех, чей первый день — когорта, сколько вернулись
-- ровно через 1, 7 и 30 дней.
WITH first AS (SELECT id, MIN(day) AS d0 FROM reports GROUP BY id)
SELECT f.d0 AS cohort,
       COUNT(*) AS installs,
       SUM(EXISTS (SELECT 1 FROM reports r WHERE r.id = f.id AND r.day = date(f.d0, '+1 day')))  AS d1,
       SUM(EXISTS (SELECT 1 FROM reports r WHERE r.id = f.id AND r.day = date(f.d0, '+7 days')))  AS d7,
       SUM(EXISTS (SELECT 1 FROM reports r WHERE r.id = f.id AND r.day = date(f.d0, '+30 days'))) AS d30
FROM first f GROUP BY f.d0 ORDER BY f.d0;

-- Сессии без сбоев (crash-free sessions), по версиям.
SELECT version,
       SUM(app_started) AS sessions,
       SUM(app_crashed) AS crashes,
       ROUND(100.0 * (1 - 1.0 * SUM(app_crashed) / MAX(SUM(app_started), 1)), 2) AS crash_free_pct
FROM reports GROUP BY version ORDER BY version;

-- Обновления: удачные и неудачные по версиям.
SELECT version, SUM(update_success) AS ok, SUM(update_failed) AS failed FROM reports GROUP BY version;

-- Активация (раздел 38): знакомство пройдено, перенос сделан, Jiyda — браузер по умолчанию,
-- активных дней ≥ 3.
SELECT COUNT(*) AS installs,
       SUM(onboarded) AS onboarded,
       SUM(imported) AS imported,
       SUM(is_default) AS default_browser,
       SUM(active_days >= 3) AS active_3_days,
       SUM(active_days >= 3 AND (imported OR is_default)) AS activated
FROM (
  SELECT id,
         MAX(onboarding_completed > 0) AS onboarded,
         MAX(import_completed > 0) AS imported,
         MAX(default_browser) AS is_default,
         COUNT(*) AS active_days
  FROM reports GROUP BY id
);

-- Чем пользуются: доля активных дней со Spaces и с палитрой.
SELECT ROUND(100.0 * AVG(workspace_used > 0), 1) AS spaces_pct,
       ROUND(100.0 * AVG(command_palette_used > 0), 1) AS palette_pct
FROM reports WHERE day >= date('now', '-30 days');
