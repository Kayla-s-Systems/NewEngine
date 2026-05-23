# PreStart Icons via AssetManager

Canonical logical paths for PreStart UI icons:

```text
ui/prestart/icons/<name>.svg
```

These files are intended to be copied into the project AssetManager/VFS asset root, for example:

```text
NewEngine/neocore2/assets/ui/prestart/icons/logo.svg
```

PreStart runs before runtime plugins, so it uses a bootstrap AssetManager-style resolver that reads the same `plugins.newengine.assets.layers` roots from `config.json` for loose assets. Runtime `engine.assets` can serve the same logical paths after startup.
