---

# Развёрнутое описание структуры `listFiles`

## 1. Назначение

`listFiles` — это базовая структура описания содержимого ассет-файла или ассет-контейнера.

Её задача — сделать любой файл в data-driven мире не просто “байтовым файлом на диске”, а словарём адресуемых entries, где каждый entry является отдельным ассетом или под-ассетом.

Главная идея:

```text
file.ext
  contains entries[]
  each entry has stable name/id/type/metadata/dependencies
  each entry can be addressed as file.ext@entry
```

То есть файл становится контейнером ассетов, а не одиночным объектом.

Примеры:

```text
player/abigail/textures/abigail.ytd@head_diff_000_a_uni
player/abigail/models/abigail.ydd@head
player/abigail/models/abigail.ydd@body_lod0
player/abigail/materials/abigail.nemat@skin_head
player/abigail/audio/voice.ynd@pain_01
world/interiors/safehouse.ytyp@bedroom_archetype
```

В одном `.ytd` может быть много текстур.
В одном `.ydd` может быть много моделей/drawables.
В одном `.nemat` может быть много материалов.
В одном `.ytyp` может быть много archetype entries.
В одном `.ynd` или другом sound dictionary может быть много звуков.
В одном `.pak` может лежать целое дерево ассетов.

`listFiles` должна стать общей manifest-структурой для AssetManager, AssetBrowser, importer pipeline, validation, dependency graph и runtime resolve.

---

## 2. Почему это нужно

Сейчас проблема классическая: движок постепенно превращается в набор ручных мест, где код сам знает:

```text
какой файл открыть;
какую модель из него взять;
какую текстуру загрузить;
какой материал создать;
куда материал повесить;
какой звук считать конкретным ассетом;
какой ytyp применить;
какие зависимости подтянуть.
```

Это не data-driven. Это procedural loading.

`listFiles` должна перевернуть модель:

```text
код не знает структуру каждого формата вручную;
формат сам объявляет manifest entries;
AssetManager видит entries как адресуемые ассеты;
resolver связывает file@entry в единый graph;
runtime получает уже resolved domain packets.
```

В итоге любой инструмент и runtime-код работают не с “особенностями конкретного файла”, а с общей моделью:

```text
AssetFileManifest
AssetEntryManifest
AssetRef = file@entry
AssetDependency
AssetKind
AssetGatewayRoute
```

---

## 3. Базовая терминология

### File

`File` — логический VFS-путь к ассет-файлу.

Пример:

```text
player/abigail/textures/abigail.ytd
```

Файл может физически лежать:

```text
loose file
inside .pak/.nepak
remote/cache layer
mod override layer
runtime cache layer
```

Но для движка это всегда один logical path.

---

### Entry

`Entry` — именованный ассет внутри файла.

Пример:

```text
head_diff_000_a_uni
head_norm_000_a_uni
body
skin_head
pain_01
player_abigail
```

Entry выбирается через `@`:

```text
file.ext@entry
```

---

### AssetRef

`AssetRef` — полная ссылка на файл или entry.

```text
AssetRef = logical_path + optional_entry_selector
```

Примеры:

```text
textures/characters/abigail.ytd@head_diff
models/characters/abigail.ydd@head
materials/characters/abigail.nemat@skin_head
```

Если `@entry` не указан, ссылка означает либо весь файл, либо default entry — только если формат явно поддерживает default entry.

Правило:

```text
implicit default entry should be avoided for authored content
```

Лучше всегда явно писать:

```text
file.ext@entry
```

---

## 4. Что такое `listFiles`

`listFiles` — это унифицированный manifest ответа от file-type handler.

Неудачное, но понятное рабочее имя: `listFiles`.
Более точные внутренние имена могут быть:

```text
AssetFileManifest
AssetDictionaryManifest
AssetEntryList
ContainerEntryManifest
FileEntryManifest
```

Но концептуально `listFiles` отвечает на вопрос:

```text
Что находится внутри этого файла, какие entries доступны, какого они типа и как их получить?
```

---