# North Star Suite Actions



> [!INFO] INFO BLOCK — назначение

> **У нас сейчас:** этот файл генерируется из Suite action descriptors и является source of truth для меню/bridge/headless запуска.

>

> **Technical details (EN):** schema=`northstar.suite_action_registry.v1`; action dispatch goes through `takesome.py <command>`.



- Actions: `48`

- Errors: `0`

- Warnings: `0`



## Build



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `plugins.build.dev` | `build-plugins dev` | yes | normal | `NewEngine/neocore2/logs/build/plugin-sync-latest.log`, `lastbuild.log` |

| `plugins.build.force.dev` | `build-plugins dev --force` | yes | normal | `NewEngine/neocore2/logs/build/plugin-sync-latest.log`, `lastbuild.log` |

| `plugins.build.release` | `build-plugins release` | yes | normal | `NewEngine/neocore2/logs/build/plugin-sync-latest.log`, `lastbuild.log` |



## Diagnostics



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `build.registry.check` | `registry-preflight` | yes | normal | `NewEngine/neocore2/buildInfo/tools/SUITE_TOOLING_REGISTRY.json`, `NewEngine/neocore2/buildInfo/tools/TOOL_AUDIT_FINDINGS.md` |

| `registry.generate` | `registry-report` | yes | normal | `NewEngine/neocore2/buildInfo/tools/TOOL_REGISTRY.json`, `NewEngine/neocore2/buildInfo/tools/SUITE_ACTIONS.json`, `NewEngine/neocore2/buildInfo/tools/TOOL_AUDIT_FINDINGS.md` |

| `suite.actions.list` | `suite-actions-list` | yes | normal | `NewEngine/neocore2/buildInfo/tools/SUITE_ACTIONS.json` |

| `suite.actions.validate` | `suite-actions-validate` | yes | normal | `NewEngine/neocore2/buildInfo/tools/SUITE_ACTIONS.md` |

| `suite.bridge.menu.generate` | `suite-bridge-menu-generate` | yes | normal | `NewEngine/neocore2/buildInfo/tools/SUITE_ACTIONS_BRIDGE_MENU.json` |



## Editor



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `gui.editor.build.dev` | `build-tool --tool-dir EngineRepo/NewEngine/editor/northstar-gui-editor` | yes | normal | `EngineRepo/NewEngine/editor/northstar-gui-editor/target/debug/northstar-gui-editor.exe` |

| `gui.editor.build.release` | `build-tool --tool-dir EngineRepo/NewEngine/editor/northstar-gui-editor --release` | yes | normal | `EngineRepo/NewEngine/editor/northstar-gui-editor/target/release/northstar-gui-editor.exe` |

| `gui.editor.doctor` | `tools run northstar.gui_editor -- doctor --root $repo_root/EngineRepo/NewEngine` | yes | normal | — |

| `gui.editor.open.texture_dictionary` | `tools run northstar.gui_editor -- open --root $repo_root/EngineRepo/NewEngine --asset assets/textures/ui/icons/builtin_icons.ytd` | yes | normal | — |

| `gui.editor.types.list` | `tools run northstar.gui_editor -- types-list --root $repo_root/EngineRepo/NewEngine` | yes | normal | — |



## Importers



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `importers.build.dev` | `build-importers dev` | yes | normal | `NewEngine/neocore2/logs/build/` |

| `importers.build.release` | `build-importers release` | yes | normal | `NewEngine/neocore2/logs/build/` |



## Metadata



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `ytyp.build` | `build-ytyp` | yes | normal | `**/*.ytyp` |

| `ytyp.inspect` | `inspect-ytyp` | yes | normal | — |

| `ytyp.validate` | `validate-ytyp` | yes | normal | — |



## Run



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `runtime.run.game.dev` | `run-game dev` | yes | normal | `NewEngine/neocore2/logs/run/` |



## Source



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `source.pack` | `pack-source` | yes | normal | `SOURCE_ARCHIVE_MANIFEST.json` |



## Suite Intelligence



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `suite.intelligence.analyze` | `suite-intelligence` | yes | normal | `.takesome/intelligence/suite-task-report.json` |

| `suite.intelligence.loop` | `suite-intelligence-loop` | yes | normal | `.takesome/intelligence/loop-state.json`, `.takesome/intelligence/loop-events.jsonl`, `.takesome/intelligence/operator-request.md`, `.takesome/intelligence/operator-response.md` |

| `suite.intelligence.loop.check` | `suite-intelligence-loop-check` | yes | normal | `.takesome/intelligence/loop-state.json`, `.takesome/intelligence/loop-events.jsonl`, `.takesome/intelligence/operator-request.md` |



## Textures



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `first_party.ddsinfo.inspect` | `tools run northstar.ddsinfo -- --library $repo_root/tools/toolbelt/libraries/northstar.ddsinfo.dll --dimensions --compression $repo_root/tools/toolbelt/first_party/northstar/ddsinfo/testData/test.dds` | yes | normal | — |

| `ytd.build` | `build-ytd` | yes | normal | `**/*.ytd` |

| `ytd.extract` | `extract-ytd` | yes | normal | `.takesome/extract/ytd/**/*.dds` |

| `ytd.inspect` | `inspect-ytd` | yes | normal | — |

| `ytd.validate` | `validate-ytd` | yes | normal | — |



## Tools



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `first_party.testAll` | `first-party-test-all` | yes | normal | `tools/toolbelt/first_party/.testAll/last-run.txt` |

| `tools.doctor` | `tools-doctor` | yes | normal | `NewEngine/neocore2/buildInfo/tools/TOOL_AUDIT_FINDINGS.md` |

| `tools.list` | `tools-list` | yes | normal | `NewEngine/neocore2/buildInfo/tools/TOOL_REGISTRY.json` |

| `tools.validate` | `tools-validate` | yes | normal | `NewEngine/neocore2/buildInfo/tools/TOOL_REGISTRY.md`, `NewEngine/neocore2/buildInfo/tools/TOOL_AUDIT_FINDINGS.md` |



## UI



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `ui.build` | `build-ui` | yes | normal | `EngineRepo/gameAssets/ui/**/*.neui` |

| `ui.inspect` | `inspect-ui` | yes | normal | — |

| `ui.validate` | `validate-ui` | yes | normal | — |



## Vendor Tools



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `third_party.testAll` | `third-party-test-all` | yes | normal | `tools/toolbelt/third_party/.testAll/last-run.txt` |

| `vendor.archive.create` | `tar-create` | no | normal | — |

| `vendor.archive.extract` | `tar-extract` | no | normal | — |

| `vendor.archive.list` | `tar-list` | no | normal | — |

| `vendor.diff.files` | `diff-files` | no | normal | — |

| `vendor.fgrep.files` | `fgrep-files` | no | normal | — |

| `vendor.gnuwin32.doctor` | `vendor-gnuwin32-doctor` | yes | normal | — |

| `vendor.merge3.files` | `diff3-files` | no | normal | — |

| `vendor.sdiff.files` | `sdiff-files` | no | normal | — |

| `vendor.sed.file` | `sed-file` | no | normal | — |

| `vendor.tail.file` | `tail-file` | no | normal | — |

| `vendor.touch.file` | `touch-file` | no | normal | — |



## Workspace



| Action | Command | Menu | Danger | Outputs |

|---|---|---:|---|---|

| `workspace.clean.full` | `clean-workspace` | yes | destructive | `NewEngine/neocore2/logs/` |



## Invariant



```text

Suite action is descriptor, not hardcoded button.

Suite shell lists registry actions and dispatches through CommandBus.

Every heavy action declares outputs and diagnostics.

```
