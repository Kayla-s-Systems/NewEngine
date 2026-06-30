# newengine-ui-navigation-api

Declarative navigation document document and action-route DTOs shared by UI providers, navigation document runtimes and host command routers.

This crate contains no product-specific navigation document behaviour and no product-specific item ids. It describes navigation documents as data:

- `UiNodeNavigationDocument`
- `UiNodeNavigationPage`
- `UiNodeNavigationItem`
- `UiNodeActionRoute`
- `UiNodeSelectionState`
- `UiNodeTransition`
- `UiNodeFeedbackEvent`

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-ui-navigation-api`

**Role:** User interface runtime assets or UI provider implementation data.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
