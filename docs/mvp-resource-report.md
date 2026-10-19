# Ресурсы MVP

**PASS: native release-центр на macOS**, запуск 18 октября 2026 года,
00:54:24.850–00:54:45.945 UTC. Наблюдённый максимальный RSS: **11.94 MiB до
входа**, **48.00 MiB во время login KDF и загрузки UI**, **15.92 MiB с двумя
TLS agents и двумя вкладками**. Это один короткий прогон на указанной машине;
ресурсный бюджет для production из него не устанавливается.

[Полные данные](assets/mvp/resources.json) содержат 323 исходные выборки,
6 native окон, 8 browser snapshots, размеры и SHA-256 артефактов. Статус
функциональной проверки поставки приведён отдельно в [приёмке MVP](mvp-acceptance.md).

## Машина и идентичность

| Параметр                                | Значение                                                           |
| --------------------------------------- | ------------------------------------------------------------------ |
| Native center / web / manifest build    | `0.1.0+b09d0d04237f.1792284226`                                    |
| Source revision / dirty                 | `b09d0d04237f` / `false`                                           |
| Source fingerprint                      | `a51a6e9f4893dca5f03536db69375d8694b8b9f281f9e2710d1c8613b35d415e` |
| Native build                            | `release`, `--no-default-features --features center`               |
| ОС / архитектура                        | macOS, Darwin `25.6.0`, `arm64` / machine `arm64`                  |
| CPU                                     | Apple M1 Pro, 10 логических ядер                                   |
| RAM, bytes                              | 17179869184                                                        |
| Свободная память до / после, bytes      | 342523904 / 154796032                                              |
| Load average до, 1 / 5 / 15 min         | 9.98681640625 / 10.48046875 / 12.0712890625                        |
| Load average после, 1 / 5 / 15 min      | 8.07275390625 / 10.0107421875 / 11.86572265625                     |
| Rust                                    | `rustc 1.99.1 (58cf8f9c3 2026-10-13)`                              |
| Bun / Playwright / Chromium             | `1.4.3` / `1.64.1` / `156.0.8078.4`                                |
| Запрошенный интервал / минимальное окно | 50 ms / 3000 ms                                                    |

Load average оставался высоким. Отдельные build/test/lifecycle процессы
завершились до запуска, но сама `measure:mvp` собирает native release
непосредственно перед sampling. Контролируемого idle-host baseline или
паузы для охлаждения машины не было. Свободная память и load average
относятся ко всей машине, а не к процессу центра.

`source` и `buildProvenance` в JSON относятся к измеренным артефактам, а не
к более позднему Git HEAD. Перед записью `PASS` harness повторно проверил
fingerprint, receipt, байты native/web артефактов, build центра/web и
manifest. Очистка собственного fixture завершилась до публикации результата.

## RSS и CPU native центра

Измерен отдельный release-процесс `hmail-cleaner`. Оба синтетических TLS
агента находятся в другом процессе: их память и CPU исключены из счётчиков
центра. RSS браузера и процесса `ps` также не включён.

| Окно                                | RSS min / mean / max, bytes              | Max, MiB | CPU time, ms | Средний CPU, % одного ядра | Samples / длительность, ms |
| ----------------------------------- | ---------------------------------------- | -------- | ------------ | -------------------------- | -------------------------- |
| Центр до входа                      | 12320768 / 12419982.222222222 / 12517376 | 11.94    | 0            | 0.000                      | 54 / 3029.35               |
| Login KDF и начальный UI            | 13615104 / 43923076.74074074 / 50331648  | 48.00    | 100          | 3.308                      | 54 / 3022.85               |
| Два TLS agents, две вкладки         | 14729216 / 16048574.836363636 / 16695296 | 15.92    | 0            | 0.000                      | 55 / 3017.72               |
| Fast preview через UI               | 15040512 / 16966262.153846152 / 17154048 | 16.36    | 20           | 0.666                      | 52 / 3001.42               |
| Full protocol preview и UI observer | 16941056 / 17294829.037037037 / 17301504 | 16.50    | 10           | 0.330                      | 54 / 3028.63               |
| Preview и отмена через UI           | 14991360 / 17103985.777777776 / 17301504 | 16.50    | 20           | 0.663                      | 54 / 3017.42               |

Строки соответствуют `native[]` в исходном порядке: `center_idle_locked`,
`center_login_kdf_and_initial_ui`, `center_two_tls_agents_two_open_ui_tabs`,
`center_fast_preview_protocol_fixture`, `center_full_preview_protocol_fixture`,
`center_cancel_protocol_fixture`.

`ps` сообщает RSS в KiB; harness переводит его в bytes. **MiB = bytes /
1048576**. Max — наибольшая наблюдённая выборка, поэтому краткий пик между
выборками мог быть пропущен. 48 MiB — sampled peak всего окна KDF/UI, а не
гарантированный мгновенный пик самого KDF.

Средний CPU = прирост накопленного CPU time / фактическое время между первой
и последней выборками × 100. Это доля одного ядра. Точность `ps` в этом
отчёте ограничена шагом CPU time 10 ms; ноль означает отсутствие различимого
прироста счётчика. Фактические интервалы выборок составили 52.63–117.09 ms,
среднее по отдельным окнам — 55.88–58.85 ms: интервал включает запуск `ps`.
В таблице округлены только MiB, CPU % и длительность; JSON сохраняет полные
значения и все `raw[]`.

Fast/full/cancel измеряют работу центра с протокольными fixtures, без
реального удаления почтовых файлов, SQL-записей и вызовов SCM. Full dispatch
выполнен через настоящий authenticated API центра, затем наблюдался в UI
после reload: общий fixture validator сообщает `running`, поэтому это окно
не подтверждает full preflight через форму.

## Клиент: JavaScript heap и DOM

Свежий Chromium context доверял только SPKI сгенерированного fixture CA.
`Runtime.getHeapUsage` измеряет **JavaScript heap**, без принудительного GC.
Это не RSS/PSS всего браузера, GPU или полного дерева browser processes.

| Снимок                   | JS heap used / total, bytes | DOM elements / nodes | Строки journal / history |
| ------------------------ | --------------------------- | -------------------- | ------------------------ |
| `initial_login`          | 2632500 / 4014080           | 58 / 131             | 0 / 0                    |
| `initial_cleanup`        | 4719804 / 7241728           | 151 / 979            | 1 / 0                    |
| `second_cleanup_tab`     | 2842152 / 5586944           | 144 / 668            | 0 / 0                    |
| `fast_preview_completed` | 6169600 / 8814592           | 156 / 1048           | 3 / 0                    |
| `full_preview_completed` | 7279556 / 9682944           | 155 / 1983           | 2 / 0                    |
| `cancelled`              | 7388848 / 10993664          | 159 / 2032           | 4 / 0                    |
| `history_first_page`     | 9655296 / 13697024          | 989 / 4493           | 0 / 50                   |
| `settings_selected`      | 10875176 / 16138240         | 113 / 5025           | 0 / 0                    |

DOM nodes получены через CDP `Memory.getDOMCounters`, элементы и строки —
через selectors страницы. Для истории создана 51 синтетическая запись;
первый экран содержит 50 строк. Эти снимки не доказывают стабильность памяти
за длительный период и не заменяют наблюдение после GC.

Все chunks ниже находятся под `/_astro/`. Суммы относятся к resource entries
соответствующего снимка; для lazy pages выбраны только дополнительные имена
относительно `initial_cleanup`.

| Загрузка JS                       | Chunks                                                                                                                                                | Encoded body, bytes | Decoded body, bytes | Transfer, bytes |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- | ------------------- | --------------- |
| Login                             | `index.CHlIyGGv.js`, `client.svelte.ybgvXSlK.js`, `client.CfqdpCeB.js`, `auth.Cc-9irn4.js`                                                            | 271078              | 271078              | 272278          |
| Cleanup после login               | `CleanupIsland.X3_J_t8v.js`, `client.svelte.ybgvXSlK.js`, `client.CfqdpCeB.js`, `auth.Cc-9irn4.js`, `ServerHeader.DjDv8n-j.js`, `display.BOIDn1v_.js` | 307242              | 307242              | 37163           |
| Дополнительно при выборе истории  | `index.BKU-dbld.js`                                                                                                                                   | 10240               | 10240               | 10540           |
| Дополнительно при выборе настроек | `index.B4btea68.js`                                                                                                                                   | 46263               | 46263               | 46563           |

Это browser-reported `PerformanceResourceTiming`: encoded/decoded — тело
ответа, transfer включает response headers. Cleanup открыт после login в
том же context; общие chunks уже в cache и имеют transfer 0. Поэтому
307242 bytes тела cleanup не означают такой же повторный сетевой transfer.
Суммы разных снимков не являются размером уникальных байтов всей сессии.
Размер всего статического JS приведён отдельно ниже.

Преждевременных запросов history/profile/session-settings до их выбора:
**0** (`lazyPages.prematureDataRequests=[]`). Первый TLS agent получил
3 PUT заданий, 1 cancel, 3 SSE subscriptions и 10 health calls; второй —
0 / 0 / 0 и 2 health calls. Это счётчики синтетического протокольного стенда.

## Артефакты и контрольные суммы

Native файл центра: **19044928 bytes**, fixture: **9682096 bytes**.
Статическое web-дерево: **418165 bytes**, из них JS: **363844 bytes**.
Для дерева нет одного файлового SHA-256: все 15 файлов, их размеры и hashes
сохранены в `buildProvenance.artifacts[]` полного JSON; перечень ниже.

| Файл                                       | Bytes    | SHA-256                                                            |
| ------------------------------------------ | -------- | ------------------------------------------------------------------ |
| `target/release/hmail-cleaner`             | 19044928 | `8ebad1b2033d0a765f3bc85a3eb85be6a1e834a4d29353649ed25446a7250c81` |
| `target/release/examples/resource_fixture` | 9682096  | `59014af1b6e192a67da8c7b4bc5040eabe6d8d6eb99917e18a1b44355abf11b6` |
| `center.oci.tar`                           | 22088192 | `b42f8d46e496efe3e789c1b7e2a2a4bfdab7680d5160f24982a5fc1feee4c502` |
| `hmail-cleaner-agent.exe`                  | 13865472 | `0caab87c0841da833b42745ae18c49c9817fc66b1666e7dfe2cce2474efac3a1` |
| `hmail-cleaner-agent.exe.sha256`           | 90       | `b62565865685d27b04a9327bccece231e872f6db7becdc5ff8fb416880df1d70` |
| Agent `manifest.json`                      | 243      | `e536c50c0058da9ba8d6ac9adb9144c19f7b6297ea2eb2add86656e5804ae967` |
| `quadlet/hmail-center.container`           | 794      | `4a24bfc90830c491c1b4a5ec781a0eeb705850b7537c88cd2db5dd7996017d94` |
| `quadlet/hmail-center-data.volume`         | 118      | `eb8e60f8aaa40e2b8195bb675761c26248f55358771c1f2039646b5841739a2b` |

Три agent-файла находятся в
`agent-packages/0.1.0+b09d0d04237f.1792284226/` release-каталога.
Все имена и контрольные суммы поставки совпадают с
[release manifest](assets/mvp/release-manifest.json).
Immutable OCI image: `localhost/hmail-center@sha256:0125a83a678169c02dc065a873c1ba5e013d062979a8817d7a7f3dceb530e62a`,
платформа `linux/amd64`; config digest
`sha256:b6b54ec81c4625b7a6c4955a3d29c395ecc87971bdba5ec9613fe3f9b8a5f58c`.
Размер OCI archive является размером файла доставки, без измерения
распакованного storage/cgroup/RSS. Размер EXE не подтверждает RSS службы Windows.

<details>
<summary>15 статических файлов: размеры и SHA-256</summary>

Пути относительно `apps/web/dist/`.

| Файл                               | Bytes  | SHA-256                                                            |
| ---------------------------------- | ------ | ------------------------------------------------------------------ |
| `_astro/CleanupIsland.X3_J_t8v.js` | 27567  | `86387c87b6b1f4e5f7dc62aea1afc2af8cfeef99051cda1a9b315133ee6a66c1` |
| `_astro/ServerHeader.DjDv8n-j.js`  | 6678   | `e443e18fa85147d073c8b4143c8da0dce8cb8764f6b75f2bc5e347f33ad72096` |
| `_astro/auth.Cc-9irn4.js`          | 228188 | `75d993b340011c0dacf680223b25a3ac3397aa16c3faeecaa1cb6dff5f6cfd29` |
| `_astro/auth.CuClWQ7d.css`         | 7619   | `35f81200dfb2c238ef900b7c3a53d979d2a82005217d0b3f9ff76896048ed0e3` |
| `_astro/client.CfqdpCeB.js`        | 41902  | `162af57c248a1f01f181f8cfe32b9fba063ea48cb8c0ccbe715610041660e648` |
| `_astro/client.svelte.ybgvXSlK.js` | 889    | `0526835bc7e30c2a4fa9b09b048dc860f1f2ff72b5f35e8fd1dc630a11a8ee89` |
| `_astro/display.BOIDn1v_.js`       | 2018   | `8a146ef060fd597f941d19eb044ad0542291831f06a588fbf27cada7986cf4b0` |
| `_astro/index.B4btea68.js`         | 46263  | `3ed3f73ffea09d15b120543a39e6032142ff9f77697026d3f1d9e46048bf2dc4` |
| `_astro/index.BKU-dbld.js`         | 10240  | `05f4acb2a56a71ee9212f170ffe8757ac11f2ab6ebd5dc5d3aebc75c23991464` |
| `_astro/index.BzwttS-3.css`        | 7623   | `af629c4686e8f87b2e0f98bf9d4e0b7bee6759b62cd2ade27c34379643120ba6` |
| `_astro/index.CHlIyGGv.js`         | 99     | `1903efcf614b01ea1577cca1a09db23039c7123cf5734b222fac26ab4d0cfa18` |
| `history/index.html`               | 9963   | `8f7ecd8f9f6cf550d2d731a938edb22e0e5db7096f938b974f1f98faa8b53698` |
| `index.html`                       | 9068   | `714bc7cc4d86c2c33e12af9ac2b2b619ec7c0c4bb13c4152ca61d9c79b6674f0` |
| `login/index.html`                 | 8656   | `f742dad26a4c6a376cf1e191895d6e24c14741320cdbbdb33e2e2379305a6e84` |
| `settings/index.html`              | 11392  | `8ab526b5fa96157b2aac44f8c08ecc5a712638bb6487d7381a466883147ab7fe` |

</details>

## Что не измерено

| Проверка                                             | Статус  | Причина                                                                                                                    |
| ---------------------------------------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------- |
| Linux rootless Podman/Quadlet RSS и CPU              | NOT RUN | Этот прогон измеряет native host; нужен отдельный контейнерный baseline на Linux с rootless Podman и user systemd/Quadlet. |
| Windows Server 2016 agent idle/cleanup/SCM RSS и CPU | NOT RUN | Native Server 2016 стенд с SCM, MySQL и файловой системой не подключён; TLS agents являются protocol fixtures.             |
| Реальный throughput fast/full/cancel                 | NOT RUN | Не выполнялись операции над настоящими mail files, SQL deletion и SCM.                                                     |
| 30-минутный slow SSE resource run                    | NOT RUN | Прогон ограничен короткими окнами; DOM/counters не доказывают длительную стабильность памяти.                              |
| Память всего дерева процессов браузера               | NOT RUN | CDP JS heap не включает RSS/PSS всего браузера, GPU и всех renderer processes.                                             |

Полные исходные причины сохранены в `notRun[]` JSON. Успешная отдельная
проверка Docker lifecycle не заменяет Podman/Quadlet resource baseline.
Сравнение с другим runtime на одинаковой нагрузке не выполнялось.

## Повторение и сохранность данных

```sh
HMAIL_MVP_RELEASE_MANIFEST=target/releases/0.1.0+b09d0d04237f.1792284226/release-manifest.json mise run measure:mvp
```

Команда готовит native release center, release fixture и web с одной
идентичностью, затем запускает harness. Для сопоставления нужны та же машина,
сходное состояние host и одинаковая методика. Прямой `measure.ts` требует
актуального `target/acceptance/measurement-build.json`; он не определяет
происхождение бинарного файла по текущему HEAD. Используются только временные
localhost fixtures; системное хранилище доверия не меняется.

В [экспорте](assets/mvp/resources.json) изменены только 16 строк путей:
`deliveryArtifacts.manifest` и 15 `staticAssets.files[].path`. Абсолютный
префикс репозитория заменён относительным путём; JSON pointers перечислены в
`exportMetadata.normalizedFields`. Обратная подстановка и глубокое сравнение
подтвердили полное равенство исходному отчёту: числа, raw samples, source,
artifact sizes и SHA-256 не изменены. Добавлена только `exportMetadata`.

SHA-256 исходного `target/acceptance/resources.json`:
`1b7ae4b809bf3d94e87c7eabd1056b299c99f4581d8eb7b8d61c8ce5173f2d20`.
SHA-256 документированного экспорта:
`1895c37a83c915f9d2ff5da07616e3ebd8a7cb54883028c996b9c5e707c57ac9`.

Методика в исходниках:
[состав отчёта](../tools/acceptance/measure.ts),
[окна нагрузки](../tools/acceptance/measurement/workload.ts),
[RSS/CPU](../tools/acceptance/measurement/counters.ts),
[browser snapshots](../tools/acceptance/measurement/browser.ts),
[проверка receipt](../tools/acceptance/measurement/build-receipt.ts),
[атомарная запись PASS](../tools/acceptance/measurement/report.ts).
