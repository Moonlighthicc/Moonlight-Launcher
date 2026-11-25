# Темы MoonLauncher

`.moontheme` — ZIP-архив с `moon-theme.json` и, при необходимости, папкой `assets/`. Старые `.cbth`, `Meta.toml` и пользовательские `theme.json` не загружаются. Если была выбрана старая тема, лаунчер покажет встроенную тёмную тему, не удаляя старые файлы. Обычные `.zip` по-прежнему используются для импорта сборок Minecraft.

## Создать и установить тему

1. Скопируйте шаблон `examples/themes/quiet-orbit/moon-theme.json`.
2. Измените `id`, `name` и нужные значения в `tokens`. `id` — постоянное имя для обновлений: строчные латинские буквы, цифры, `-` и `_`, до 64 символов. Первый символ — буква или цифра. Импорт с тем же `id` обновляет установленную тему.
3. Упакуйте манифест и ресурсы в ZIP, поменяйте расширение на `.moontheme`. Импортируйте файл в разделе тем или перетащите его в окно лаунчера.
4. Для редактирования на месте изменяйте манифест в папке установки темы. Лаунчер следит за файлами активной темы и перечитывает их при изменении.

Под Windows можно собрать пример из корня проекта в PowerShell:

```powershell
Compress-Archive -Path .\examples\themes\quiet-orbit\* -DestinationPath .\quiet-orbit.zip
Rename-Item -Path .\quiet-orbit.zip -NewName quiet-orbit.moontheme
```

## Что можно менять

| Токены | Назначение |
| --- | --- |
| `--bg-main`, `--bg-sidebar`, `--bg-card`, `--bg-input` | Основные поверхности |
| `--text-primary`, `--text-secondary`, `--text-muted` | Заголовки, подписи и вспомогательный текст |
| `--accent`, `--accent-hover`, `--accent-text`, `--accent-rgb` | Главные действия и их состояния |
| `--border-color`, `--selected-border`, `--selected-surface`, `--focus-ring` | Рамки, выделение и фокус клавиатуры |
| `--border-radius-sm`, `--border-radius`, `--border-radius-lg` | Контролы (8 px), карточки (12 px) и панели (16 px) по умолчанию |
| `--space-xs` … `--space-xl`, `--button-height`, `--icon-button-size` | Отступы и размеры общих кнопок |
| `--font-family`, `--font-size-base`, `--font-size-control` | Типографика |

Неуказанные токены берутся из базовых стилей. Полный набор значений по умолчанию находится в `src/styles/shared/reset.css`; для первой темы достаточно изменить палитру из примера.

Обязательные поля: `format: "moonlauncher"`, `format_version: 1`, `id` и `name`. `author`, `version` и `tokens` необязательны. Манифест также принимает `background: { "path": "assets/wallpaper.webp", "opacity": 0.25, "blur": 2 }`, `fonts: [{ "family": "My Font", "src": "assets/font.woff2" }]` и `icons: { "nav:settings": "assets/settings.svg" }`. Для выбора шрифта задайте подходящий `--font-family`. Ресурсы должны быть внутри пакета. Пакеты не могут загружать произвольные CSS-файлы или менять структуру интерфейса.