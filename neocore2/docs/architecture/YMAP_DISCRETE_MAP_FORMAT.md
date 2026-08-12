# YMAP v2 — Discrete Replaceable Map Format

> [!NOTE] REQUEST NOTE — унификация игровой карты
> **У нас сейчас:** `.ymap` уже является NEF8/ListFile и зарегистрирован за `engine.assets.maps`, но game-ready authoring исторически складывал большой runtime profile внутрь одного map body.
> **Было бы здорово:** карта должна быть только декларацией topology + cell placements. Player/gameplay/environment/material/render settings не являются map ownership и должны подключаться отдельными Definition Entries/gateways.
> **Technical details (EN):** canonical selectors: `maps/world.ymap@map`, `maps/world.ymap@cell/x/z`; DTOs: `MapIndexV1`, `MapCellV1`, `MapPlacementV1`; gateway: `engine.assets.maps`; capability: `assets.maps.backend`.

## Цель

Карта должна быть **дискретной**, **адресуемой по частям** и **заменяемой без изменения CoreEngine**.

```text
Engine owns routing.
engine.assets owns bytes/VFS/NEF8 decode.
engine.assets.maps owns YMAP semantics.
engine.scene / engine.world instantiate resolved map DTOs.
.ytyp owns reusable definition metadata.
```

Каноническая цепочка:

```text
maps/city.ymap@map
  -> MapIndexV1
  -> cells[]
      -> maps/city.ymap@cell/0/0
      -> maps/city.ymap@cell/0/1
      -> maps/city.ymap@cell/1/0
  -> MapCellV1
      -> placements[]
          -> definitions/world.ytyp@street_lamp
          -> definitions/world.ytyp@building_a
```

## Формат

`.ymap` остаётся **одним NEF8/ListFile format**. Новый top-level magic не вводится.

```text
NEF8
  content_kind = YMAP
  content_schema_version = 2
  entries:
    map
    cell/0/0
    cell/0/1
    cell/1/0
    ...
```

### `@map`

Root entry хранит только topology и ссылки на independently addressable cells.

```text
MapIndexV1
  schema = newengine.map.index.v1
  map_id
  origin[x,y,z]
  cell_size
  cells[]
    coord{x,z}
    entry = cell/x/z
    required
  layers[]
    id
    map_ref = another.ymap@map
    mode = additive | override
    priority
  tags[]
  metadata{}
```

### `@cell/x/z`

Cell entry содержит только placements этой пространственной области.

```text
MapCellV1
  schema = newengine.map.cell.v1
  coord{x,z}
  placements[]
    id
    definition_ref = *.ytyp@entry
    transform
    apply_mode
    tags[]
    enabled
```

## Почему placement ссылается только на `.ytyp@entry`

YMAP не должен знать конкретные `.ydd`, `.nemat`, `.ytd`, physics backend или renderer implementation.

Правильно:

```text
YMAP placement
  -> .ytyp@entry
  -> definition dependencies
      -> .ydd@drawable
      -> .nemat@material
      -> .ytd@texture
      -> collision/AI/streaming declarations
```

Неправильно:

```text
YMAP
  -> hardcoded .ydd
  -> hardcoded material
  -> renderer object
  -> physics body
```

Это сохраняет `Engine as Host`, `Service as Plugin` и provider replacement.

## Дискретность

`cell_size` определяет world partition grid. Координата вычисляется детерминированно:

```text
cell.x = floor((world.x - origin.x) / cell_size)
cell.z = floor((world.z - origin.z) / cell_size)
```

Замена участка мира:

```text
before:
  city.ymap@cell/4/-2 -> base provider/content

after mod override:
  city.ymap@cell/4/-2 -> modded cell payload
```

Root index и соседние cells не обязаны меняться.

## Layer composition

Map layer — ссылка на другую карту, а не копия её данных.

```text
base.ymap@map
  + dlc.ymap@map       mode=additive priority=100
  + user_mod.ymap@map  mode=override priority=500
```

Фактический выбор/override должен оставаться deterministic и диагностируемым. Built-in map content не получает скрытой привилегии только потому, что он first-party.

## Authoring XML

XML остаётся authoring presentation, а не ownership model. Canonical v2 source:

```xml
<YmapMapDefinition schema="newengine.map.definition.v2"
                   representation="xml"
                   body_format="newengine.xml.metadata.v1">
  <map id="city" cell_size="64" origin="0,0,0">
    <cells>
      <Cell x="0" z="0">
        <placements>
          <Placement
              id="lamp_001"
              definition_ref="definitions/city.ytyp@street_lamp"
              position="4,0,8"
              rotation_ypr="0,0,0"
              scale="1,1,1"
              apply_mode="instantiate" />
        </placements>
      </Cell>
    </cells>
  </map>
</YmapMapDefinition>
```

Packer/compiler обязан раскладывать этот source в addressable `@map` + `@cell/x/z` entries. Runtime не должен зависеть от физического source XML path.

## Что не принадлежит YMAP

Не хранить inline в map core schema:

```text
player controller tuning
input bindings
mission state machine
HUD strings
renderer quality settings
shadow implementation settings
physics backend tuning
material definitions
texture payloads
model payloads
time/weather implementation
```

Эти данные принадлежат своим gateways / Definition Entries / profile data.

## Validation invariants

```text
map_id != empty
cell_size > 0 and finite
origin finite
cell coordinates unique
cell entry selectors unique
layer ids unique
layer refs are .ymap@entry
placement ids unique inside cell
placement definition_ref is .ytyp@entry
transform values finite
scale > 0
```

## Runtime boundary

```text
engine.assets
  -> NEF8 bytes/body
  -> engine.assets.maps
  -> MapIndexV1 / MapCellV1 DTO
  -> engine.scene / engine.world
  -> instantiate/apply stage
```

Provider не получает `&mut World`. Map service только разрешает и валидирует declarations; world mutation выполняет scene/world apply stage.

## Blender import / map replacement

`.blend` является **authoring source**, а не runtime asset. Runtime никогда не должен подключать Blender SDK или читать `.blend` напрямую.

Канонический pipeline:

```text
level.blend
  -> Blender headless exporter
  -> one local-space OBJ per unique mesh + placement manifest
  -> models/maps/<map>/<mesh>.ydd
  -> definitions/maps/<map>/<mesh>.ytyp
  -> maps/<map>.ymap
```

Обычный Blender `MESH` импортируется без обязательной специальной разметки. Один mesh datablock становится отдельным reusable `.ydd/.ytyp`, а каждый Blender object становится только placement в соответствующей YMAP cell. Поэтому сотни инстансов одного mesh не дублируют геометрию.

Быстрая замена карты:

```bat
tools\import_blender_map.bat D:\Levels\city.blend
```

Повторный импорт с тем же `--output` заменяет тот же logical map asset и детерминированно обновляет generated model/definition records. По умолчанию команда сразу собирает runtime `.ydd/.ytyp/.ymap`; `--no-build` нужен только для authoring-only режима. Старые generated records этого map id удаляются из native build plan перед добавлением новых.

Опциональные Blender custom properties:

```text
ns_definition = definitions/shared.ytyp@street_lamp
    Reuse existing definition; geometry for this object is not imported again.

ns_material = materials/city.nemat@concrete
    Material ref for the generated definition.

ns_collision = true
    Marks generated definition with static collision intent.

ns_map_ignore = true
    Excludes the object from map import.

ns_unique_mesh = true
    Forces a separate runtime mesh when objects share one Blender mesh datablock but have object-specific modifiers.

ns_apply_mode = instantiate
    Explicit placement apply mode.

ns_tags = road,prop,streaming
    Placement tags.

ns_cell_x / ns_cell_z
    Optional explicit cell override. Without it the importer derives the cell from world position.
```

Generated source ownership:

```text
gameAssets/models/source/maps/<map>/<mesh>.obj
gameAssets/models/maps/<map>/<mesh>.ydd
gameAssets/definitions/source/maps/<map>/<mesh>.ytyp.xml
gameAssets/definitions/maps/<map>/<mesh>.ytyp
gameAssets/maps/source/imported/<map>.ymap.xml
gameAssets/maps/source/imported/<map>.import.json
gameAssets/maps/<map>.ymap
```

Импорт зарегистрирован как `northstar.importer.blender_map.v1` и принадлежит `engine.assets.maps`. Geometry compilation остаётся у YDD importer, definition semantics — у `engine.assets.definitions`, а runtime map semantics — у `engine.assets.maps`.

## Result

Главная единица карты теперь не «гигантский профиль уровня», а **маленькая адресуемая ячейка**.

```text
Map = index + cells + external definitions.
```

Это позволяет независимо:

- стримить cells;
- заменять cells модами;
- диффить изменения;
- кешировать cells;
- переиспользовать `.ytyp` definitions;
- загружать только нужную часть мира;
- диагностировать точную map entry, которая породила объект.
