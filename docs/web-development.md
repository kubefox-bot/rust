# Разработка веб-интерфейса

`apps/web` — Astro static + Svelte 5. Rust раздаёт четыре страницы и API на одном
HTTPS origin; Bun нужен при разработке и сборке. Node.js в PATH не требуется:
генерация контрактов и Astro build проверены без него.

```sh
mise install
mise exec -- bun install --frozen-lockfile
mise run contracts:check
mise run web:check
mise run web:build
```

`web:check` запускает Astro, Svelte/TypeScript и Bun unit tests. Для браузерной
проверки статической оболочки:

```sh
mise exec -- bun run --cwd apps/web browser:install
mise exec -- bun run --cwd apps/web test:browser
```

Установка Chromium требует исходящего интернета. Скрипт `browser:install`
отключает автоматическое удаление кеша браузеров других проектов. Браузерные
тесты запускаются через Playwright API под Bun; пользовательские вкладки и
профили браузера не используются. Сборка производится перед браузерными тестами.

## Контракты

Rust exporter `crates/protocol/examples/export_browser_schema.rs` содержит
явный список browser DTO. `mise run contracts:generate` обновляет schema,
TypeScript и Ajv standalone validators; `contracts:check` проверяет TypeScript
генератора и побайтную воспроизводимость всех четырёх файлов.

Секретные данные не передаются сборке. Контракт формы SSH допускает ввод пароля
или закрытого SSH-ключа оператора; private identity агента, ключ CA и внутренний
payload очистки не экспортируются. Counter передаётся канонической десятичной
строкой; валидатор дополнительно проверяет диапазон u64 через BigInt.

## Статика и локальный Rust

По умолчанию центр читает статику из `/opt/hmail/web` внутри образа. Для локальной
сборки задайте в `center.json` абсолютный `static_dir`, указывающий на
`apps/web/dist`. Каталог должен содержать все четыре страницы. API, история и
настройки работают через этот же Rust HTTPS origin.

HTML имеет `Cache-Control: no-store`; сборочные assets — immutable cache.
Маршруты `/`, `/login`, `/history`, `/settings` и варианты с завершающим `/`
раздаются явно. Неизвестные API и пути не получают SPA fallback. Asset-маршрут
проверяет канонический путь и отказывает при выходе из каталога.

Основной остров загружается с `client:load`. History/settings имеют отдельные
страницы, обычные MPA-переходы и отключённый prefetch. Перед выдачей HTML Rust
проверяет сессию и зашифрованную HttpOnly cookie профиля, затем выбирает
фиксированный маркер шапки `guest` или `admin`. Ключи, токены, логин и другие
поля cookie в HTML не подставляются; JavaScript cookie профиля не читает.
Приватные данные островов по-прежнему загружаются после проверки сессии API.
Персонализированный HTML не использует файловые ответы 206/304: Range и
условные запросы получают полный ответ, HEAD — только заголовки.
`PUBLIC_BUILD_VERSION` —
единственное публичное значение сборки, проверяется в `src/config/build.ts`.
