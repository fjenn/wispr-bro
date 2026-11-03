<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Translation review guide

This guide is for the people (and review agents) who turn Wispr Bro's
machine translations into reviewed ones, locale by locale, before 1.0
(R5 §5.6, S-37). It holds the glossary, the tone rules and the procedure
for clearing a `machine` or `untranslated` flag.

## How the strings are stored

- English is the source: `src/i18n/locales/en/fork.json`. Every other
  locale has a `fork.json` with the same keys and a sidecar
  `fork.flags.json` that marks what nobody has reviewed yet:
  - `"machine"`: translated by the local model (`llama-server`, Qwen3-4B
    Instruct), never read by a person;
  - `"untranslated"`: a copy of the English string;
  - no entry: reviewed. A reviewed string is never overwritten by the
    scripts.
- `rust.*` keys are the tray, app menu, notifications, window titles and
  VoiceOver announcements (`src-tauri/src/fork/i18n.rs` embeds the same
  files). They follow exactly the same rules.
- `overrides.*` keys rebrand Handy's own strings (F-01,
  `scripts/wb-brand-overrides.ts`); their flags are reviewed the same way.
- Handy's `translation.json` files are upstream's and are not reviewed here.

Tools (`scripts/wb-translate-missing.ts`):

| Command                                                    | What it does                                                                   |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `bun scripts/wb-translate-missing.ts --review de`          | The worklist: every flagged key as `key, flag, English, current value, hint`.  |
| `bun scripts/wb-translate-missing.ts --check --locale de`  | Parity, plural forms and placeholders for one locale.                          |
| `bun scripts/wb-translate-missing.ts --strict --locale de` | Fails while the locale has any flag left.                                      |
| `bun run wb:check:translations`                            | The CI parity check, all locales.                                              |
| `bun run wb:check:translations:strict`                     | The CI strict step; enforced only when `ci/i18n-strict` says `I18N_STRICT=on`. |

## Procedure: clearing a flag

Work one locale per commit.

1. Run `bun scripts/wb-translate-missing.ts --review <lang>` and work down
   the list. Read the English source column, not the key name: plural forms
   of other languages are written from English `_other` (see Plurals).
2. For each string, check:
   - **Meaning.** It says what the English says, no more. When the English
     is unclear, find the call site (`grep -rn '"<key>"' src/fork
src-tauri/src/fork`) or open the screen.
   - **Glossary and tone.** The terms below and the locale's form of
     address. Many machine strings mix „du“ and „Sie“ or translate one
     feature three ways; fix that.
   - **Kept as is.** "Wispr Bro", "Polish", "Prompt Engineer", model and
     Apple product names, key names and symbols (fn, ⌥, ⌘, ⌃, ⇧, Esc,
     Space, Tab, Return), `{{placeholders}}` (same names, same
     `, number` formats) and `<tags>`. The checks reject a changed
     placeholder.
   - **Plural form.** A `_few`/`_many`/`_two`/`_zero` key must be the
     grammatical form for the counts in the hint column; `_one` in `ru`,
     `uk`, `fr`, `pt`, `hi` also covers other numbers (21, 0, …), so it
     needs the count, not "one" / "a".
   - **Length.** Buttons, menu items, tabs and the bar have little room.
     Keep labels about as long as the English; prefer the shorter of two
     correct words.
   - **Punctuation and typography** of the language: quotation marks
     („…“, «…», 「…」), no space before `:` except in French (narrow
     no-break space before `: ; ! ?`), Chinese and Japanese full-width
     punctuation, ellipsis `…` as one character.
3. Edit the value in `src/i18n/locales/<lang>/fork.json` if needed, then
   delete the key's line from `src/i18n/locales/<lang>/fork.flags.json`.
   Deleting the flag is what "reviewed" means; never delete it without
   having read the string. An `untranslated` string whose English is also
   right in the language (rare: "OK", "Mac") may be kept and cleared.
4. Look at the screens you touched when you can: `bun run dev`, set the
   app language in Settings › General, or run the Playwright fork suite
   with the locale (`tests/fork/` specs set it through the settings mock).
   Arabic and Hebrew are right to left; check that the sentence still reads
   correctly with embedded Latin words and numbers.
5. Run `bun scripts/wb-translate-missing.ts --check --locale <lang>` and
   `bun run wb:check:translations`, then commit:
   `i18n(<lang>): review <area> strings` with `git commit -s`.
6. When `--strict --locale <lang>` passes, the locale is done. When every
   locale passes, the owner sets `I18N_STRICT=on` in `ci/i18n-strict`.

Rules for reviewers:

- R5 asks a person (not only an agent) to review the 10 sample-text
  languages: en, es, fr, de, pt, it, ja, zh, ko, ru. The other 15 may ship
  machine-translated after the S-37 pass; the release notes list them.
- Never edit English in a review commit. A wrong or unclear English string
  is a separate fix (and S-37a's copy check).
- When English changes, the scripts drop the old translation only if the
  key changes. A slice that rewrites English text must rename the key (or
  remove the old translations) so the flag comes back.
- New strings arrive copy-English (`untranslated`) from slices and are
  machine-translated once per merge wave (`--backend llama-server`); they
  then need review like everything else.

## Plurals

A string that shows a count is a group of keys with i18next's plural
suffixes: English has `<key>_one` and `<key>_other`; every other locale has
exactly its CLDR cardinal categories, which the checks enforce (an extra or
missing form fails). The call site passes `count` (React:
`t("key", { count })`; Rust: `i18n::t_plural`). Some messages show the
number through another placeholder (`{{goal}}`, `{{max}}`, `{{seconds}}`)
and pass `count` only to pick the form.

A locale's `_one` is machine-translated from English `_one` (the singular)
unless that drops a placeholder the locale needs (English "A recording was
saved" cannot serve Russian, whose "one" also covers 21); every other form
comes from English `_other`. The counts below go to the model as a hint. Where a message has two numbers, the plural form follows `count`;
the other number is a constant (for example the 150 words a minute of
speech) and is written in the form that fits it.

| Locale  | Forms (examples)                                                                                                  |
| ------- | ----------------------------------------------------------------------------------------------------------------- |
| `ar`    | `_zero` (0), `_one` (1), `_two` (2), `_few` (3, 4, 5, 6), `_many` (11, 12, 13, 14), `_other` (100, 101, 102, 200) |
| `bg`    | `_one` (1), `_other` (0, 2, 3, 4)                                                                                 |
| `ca`    | `_one` (1), `_many` (1000000), `_other` (0, 2, 3, 4)                                                              |
| `cs`    | `_one` (1), `_few` (2, 3, 4), `_many` (0.5, 1.5, 2.5), `_other` (0, 5, 6, 7)                                      |
| `da`    | `_one` (1, 0.5, 1.5), `_other` (0, 2, 3, 4)                                                                       |
| `de`    | `_one` (1), `_other` (0, 2, 3, 4)                                                                                 |
| `es`    | `_one` (1), `_many` (1000000), `_other` (0, 2, 3, 4)                                                              |
| `fr`    | `_one` (0, 1, 0.5, 1.5), `_many` (1000000), `_other` (2, 3, 4, 5)                                                 |
| `he`    | `_one` (1, 0.5), `_two` (2), `_other` (0, 3, 4, 5)                                                                |
| `hi`    | `_one` (0, 1, 0.5), `_other` (2, 3, 4, 5)                                                                         |
| `id`    | `_other` (0, 1, 2, 3)                                                                                             |
| `it`    | `_one` (1), `_many` (1000000), `_other` (0, 2, 3, 4)                                                              |
| `ja`    | `_other` (0, 1, 2, 3)                                                                                             |
| `ko`    | `_other` (0, 1, 2, 3)                                                                                             |
| `ne`    | `_one` (1), `_other` (0, 2, 3, 4)                                                                                 |
| `nl`    | `_one` (1), `_other` (0, 2, 3, 4)                                                                                 |
| `pl`    | `_one` (1), `_few` (2, 3, 4, 22), `_many` (0, 5, 6, 7), `_other` (0.5, 1.5, 2.5)                                  |
| `pt`    | `_one` (0, 1, 0.5, 1.5), `_many` (1000000), `_other` (2, 3, 4, 5)                                                 |
| `ru`    | `_one` (1, 21, 31, 41), `_few` (2, 3, 4, 22), `_many` (0, 5, 6, 7), `_other` (0.5, 1.5, 2.5)                      |
| `sv`    | `_one` (1), `_other` (0, 2, 3, 4)                                                                                 |
| `tr`    | `_one` (1), `_other` (0, 2, 3, 4)                                                                                 |
| `uk`    | `_one` (1, 21, 31, 41), `_few` (2, 3, 4, 22), `_many` (0, 5, 6, 7), `_other` (0.5, 1.5, 2.5)                      |
| `vi`    | `_other` (0, 1, 2, 3)                                                                                             |
| `zh`    | `_other` (0, 1, 2, 3)                                                                                             |
| `zh-TW` | `_other` (0, 1, 2, 3)                                                                                             |

`_many` in Catalan, Spanish, French, Italian and Portuguese is the form for
round millions ("1 000 000 de mots"); `_other` in Czech, Polish, Russian and
Ukrainian is for fractions. The app shows neither in practice, but i18next
asks for them, so they exist and need a sensible translation.

## Tone

Address the user the way macOS does in that language, so the app feels
native next to System Settings. Short, direct sentences; buttons and menu
items are verbs (or the language's usual infinitive form); no exclamation
marks except in celebrations; no slang.

| Locale  | macOS UI | Tone                                                                                                                                                             |
| ------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ar`    | yes      | Modern Standard Arabic, neutral and polite; address the user in the masculine singular imperative as macOS does, avoid gendered phrasing where a noun works.     |
| `bg`    | yes      | Formal „Вие“ forms (macOS Bulgarian addresses the user formally); short imperatives on buttons.                                                                  |
| `ca`    | yes      | Informal „tu“, as macOS Catalan does; imperatives on buttons.                                                                                                    |
| `cs`    | yes      | Formal „vy“ forms, as macOS Czech does; infinitives or imperatives on buttons.                                                                                   |
| `da`    | yes      | Informal „du“, as macOS Danish does.                                                                                                                             |
| `de`    | yes      | Informal „du“ (lower-case „du/dein“), as macOS German does since macOS 11; never „Sie“.                                                                          |
| `es`    | yes      | Informal „tú“, as macOS Spanish does; neutral Spanish, no regional slang.                                                                                        |
| `fr`    | yes      | Formal „vous“, as macOS French does; infinitives on buttons.                                                                                                     |
| `he`    | yes      | Address the user in the masculine singular as macOS Hebrew does, preferring infinitives and nouns that avoid gender.                                             |
| `hi`    | yes      | Polite „आप“, as macOS Hindi does; common English tech words in Devanagari are fine where Apple uses them.                                                        |
| `id`    | yes      | Polite „Anda“, as macOS Indonesian does.                                                                                                                         |
| `it`    | yes      | Informal „tu“, as macOS Italian does.                                                                                                                            |
| `ja`    | yes      | Polite です/ます style as macOS Japanese uses; avoid pronouns for the user; buttons are short verbs or nouns.                                                    |
| `ko`    | yes      | Polite 합니다/하세요 style as macOS Korean uses; avoid pronouns for the user.                                                                                    |
| `ne`    | no       | Polite „तपाईं“. macOS has no Nepali UI, so follow the Hindi column's choices where Nepali usage allows.                                                          |
| `nl`    | yes      | Informal „je/jij“, as macOS Dutch does.                                                                                                                          |
| `pl`    | yes      | Informal second person („ty“, mostly implied by the verb), as macOS Polish does; infinitives on buttons.                                                         |
| `pt`    | yes      | Brazilian Portuguese with „você“, as macOS pt-BR does.                                                                                                           |
| `ru`    | yes      | Formal „вы“ (lower case), as macOS Russian does.                                                                                                                 |
| `sv`    | yes      | Informal „du“, as macOS Swedish does.                                                                                                                            |
| `tr`    | yes      | Formal „siz“, as macOS Turkish does.                                                                                                                             |
| `uk`    | yes      | Formal „ви“ (lower case), as macOS Ukrainian does.                                                                                                               |
| `vi`    | yes      | Neutral „bạn“, as macOS Vietnamese does.                                                                                                                         |
| `zh`    | yes      | Simplified Chinese for mainland China; „你“ as current macOS uses; no spaces between Chinese characters, one space around Latin words and numbers as Apple does. |
| `zh-TW` | yes      | Traditional Chinese for Taiwan; „你“ as current macOS uses; Taiwan vocabulary (設定, 檔案, 鍵盤快速鍵), not Hong Kong or mainland terms.                         |

"macOS UI: no" means Apple ships no macOS interface in that language, so
there is no Apple wording to follow; use the most common computing terms in
that language.

## Glossary

The same data feeds the translation model's prompt
(`scripts/wb-translate-glossary.ts`); change both together (a test checks
that this page lists every term). Terms follow Apple's macOS wording where
macOS has the concept (Dictation, Settings, Notifications, keyboard
shortcut, History); for the app's own features the column gives the plain
word to use everywhere.

What the English terms mean in the app:

- **Dictation / dictate:** speaking text into any app; also the name of the
  Hub's home page.
- **Transform:** a rewrite of selected text with a shortcut (Polish and
  Prompt Engineer are the built-in transforms; their names stay English).
- **Scratchpad:** the floating note window.
- **snippet:** a saved text that a spoken phrase inserts.
- **dictionary:** the user's words and replacements.
- **the bar:** the small floating bar at the bottom of the screen (the
  "pill" in the code).
- **History, Insights, Style:** Hub pages (past dictations, statistics,
  writing style per app).
- **cleanup:** removing fillers and fixing punctuation after dictation.

| Locale                      | Dictation    | dictate          | Transform       | Scratchpad      | snippet         | dictionary   | the bar    |
| --------------------------- | ------------ | ---------------- | --------------- | --------------- | --------------- | ------------ | ---------- |
| `ar` Arabic                 | الإملاء      | أملِ             | تحويل           | المسودة         | مقتطف           | القاموس      | الشريط     |
| `bg` Bulgarian              | Диктовка     | диктувайте       | Трансформация   | Чернова         | фрагмент        | речник       | лентата    |
| `ca` Catalan                | Dictat       | dictar           | Transformació   | Esborrany       | fragment        | diccionari   | la barra   |
| `cs` Czech                  | Diktování    | diktovat         | Transformace    | Poznámkový blok | úryvek          | slovník      | lišta      |
| `da` Danish                 | Diktering    | diktere          | Transformation  | Kladdeblok      | tekststykke     | ordbog       | bjælken    |
| `de` German                 | Diktat       | diktieren        | Transformation  | Notizblock      | Textbaustein    | Wörterbuch   | die Leiste |
| `es` Spanish                | Dictado      | dictar           | Transformación  | Bloc de notas   | fragmento       | diccionario  | la barra   |
| `fr` French                 | Dictée       | dicter           | Transformation  | Bloc-notes      | fragment        | dictionnaire | la barre   |
| `he` Hebrew                 | הכתבה        | להכתיב           | המרה            | טיוטה           | קטע טקסט        | מילון        | הסרגל      |
| `hi` Hindi                  | श्रुतलेख     | बोलकर लिखें      | रूपांतरण        | स्क्रैचपैड      | स्निपेट         | शब्दकोश      | बार        |
| `id` Indonesian             | Dikte        | mendikte         | Transformasi    | Coretan         | cuplikan        | kamus        | bilah      |
| `it` Italian                | Dettatura    | dettare          | Trasformazione  | Blocco note     | frammento       | dizionario   | la barra   |
| `ja` Japanese               | 音声入力     | 音声入力する     | 書き換え        | メモパッド      | スニペット      | 辞書         | バー       |
| `ko` Korean                 | 받아쓰기     | 받아쓰기하다     | 변환            | 스크래치패드    | 스니펫          | 사전         | 바         |
| `ne` Nepali                 | श्रुतलेखन    | बोलेर लेख्नुहोस् | रूपान्तरण       | मस्यौदा         | अंश             | शब्दकोश      | बार        |
| `nl` Dutch                  | Dicteren     | dicteren         | Transformatie   | Kladblok        | tekstfragment   | woordenboek  | de balk    |
| `pl` Polish                 | Dyktowanie   | dyktować         | Przekształcenie | Brudnopis       | fragment tekstu | słownik      | pasek      |
| `pt` Portuguese             | Ditado       | ditar            | Transformação   | Rascunho        | trecho          | dicionário   | a barra    |
| `ru` Russian                | Диктовка     | диктовать        | Преобразование  | Черновик        | фрагмент        | словарь      | панель     |
| `sv` Swedish                | Diktering    | diktera          | Omvandling      | Kladdblock      | textstycke      | ordlista     | listen     |
| `tr` Turkish                | Dikte        | dikte etmek      | Dönüştürme      | Karalama        | metin parçası   | sözlük       | çubuk      |
| `uk` Ukrainian              | Диктування   | диктувати        | Перетворення    | Чернетка        | фрагмент        | словник      | панель     |
| `vi` Vietnamese             | Đọc chính tả | đọc chính tả     | Chuyển đổi      | Bản nháp        | đoạn văn bản    | từ điển      | thanh      |
| `zh` Simplified Chinese     | 听写         | 听写             | 转换            | 草稿板          | 文本片段        | 词典         | 浮动栏     |
| `zh-TW` Traditional Chinese | 聽寫         | 聽寫             | 轉換            | 草稿板          | 文字片段        | 字典         | 浮動列     |

| Locale                      | History      | Insights      | Style    | cleanup       | keyboard shortcut         | Settings      | notification  |
| --------------------------- | ------------ | ------------- | -------- | ------------- | ------------------------- | ------------- | ------------- |
| `ar` Arabic                 | السجل        | الإحصائيات    | النمط    | التنظيف       | اختصار لوحة المفاتيح      | الإعدادات     | إشعار         |
| `bg` Bulgarian              | История      | Статистика    | Стил     | почистване    | клавишна комбинация       | Настройки     | известие      |
| `ca` Catalan                | Historial    | Estadístiques | Estil    | neteja        | drecera de teclat         | Configuració  | notificació   |
| `cs` Czech                  | Historie     | Statistiky    | Styl     | vyčištění     | klávesová zkratka         | Nastavení     | oznámení      |
| `da` Danish                 | Historik     | Statistik     | Stil     | oprydning     | tastaturgenvej            | Indstillinger | meddelelse    |
| `de` German                 | Verlauf      | Statistik     | Stil     | Bereinigung   | Tastaturkurzbefehl        | Einstellungen | Mitteilung    |
| `es` Spanish                | Historial    | Estadísticas  | Estilo   | limpieza      | función rápida de teclado | Ajustes       | notificación  |
| `fr` French                 | Historique   | Statistiques  | Style    | nettoyage     | raccourci clavier         | Réglages      | notification  |
| `he` Hebrew                 | היסטוריה     | סטטיסטיקה     | סגנון    | ניקוי         | קיצור מקלדת               | הגדרות        | עדכון         |
| `hi` Hindi                  | इतिहास       | आँकड़े        | शैली     | सफ़ाई         | कीबोर्ड शॉर्टकट           | सेटिंग्ज़     | सूचना         |
| `id` Indonesian             | Riwayat      | Statistik     | Gaya     | pembersihan   | pintasan papan ketik      | Pengaturan    | pemberitahuan |
| `it` Italian                | Cronologia   | Statistiche   | Stile    | pulizia       | abbreviazione da tastiera | Impostazioni  | notifica      |
| `ja` Japanese               | 履歴         | 統計          | スタイル | 整形          | キーボードショートカット  | 設定          | 通知          |
| `ko` Korean                 | 기록         | 통계          | 스타일   | 정리          | 키보드 단축키             | 설정          | 알림          |
| `ne` Nepali                 | इतिहास       | तथ्याङ्क      | शैली     | सफाइ          | किबोर्ड सर्टकट            | सेटिङहरू      | सूचना         |
| `nl` Dutch                  | Geschiedenis | Statistieken  | Stijl    | opschonen     | toetscombinatie           | Instellingen  | melding       |
| `pl` Polish                 | Historia     | Statystyki    | Styl     | porządkowanie | skrót klawiszowy          | Ustawienia    | powiadomienie |
| `pt` Portuguese             | Histórico    | Estatísticas  | Estilo   | limpeza       | atalho de teclado         | Ajustes       | notificação   |
| `ru` Russian                | История      | Статистика    | Стиль    | очистка       | сочетание клавиш          | Настройки     | уведомление   |
| `sv` Swedish                | Historik     | Statistik     | Stil     | rensning      | kortkommando              | Inställningar | notis         |
| `tr` Turkish                | Geçmiş       | İstatistikler | Stil     | temizleme     | klavye kestirmesi         | Ayarlar       | bildirim      |
| `uk` Ukrainian              | Історія      | Статистика    | Стиль    | очищення      | клавіатурне скорочення    | Параметри     | сповіщення    |
| `vi` Vietnamese             | Lịch sử      | Thống kê      | Kiểu     | dọn dẹp       | phím tắt                  | Cài đặt       | thông báo     |
| `zh` Simplified Chinese     | 历史记录     | 统计          | 风格     | 清理          | 键盘快捷键                | 设置          | 通知          |
| `zh-TW` Traditional Chinese | 歷史記錄     | 統計          | 風格     | 清理          | 鍵盤快速鍵                | 設定          | 通知          |
