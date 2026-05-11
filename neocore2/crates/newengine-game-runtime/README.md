# newengine-game-runtime

Thin standalone game profile crate.

Responsibilities:

- register engine runtime modules;
- mount game asset roots;
- bootstrap the selected playable scene after engine plugins are ready;
- disable editor UI by returning `UiProviderKind::Null`.

Non-responsibilities:

- renderer implementation;
- Vulkan resource creation;
- shader/pipeline decisions;
- shadows and post effects;
- editor scene IO fallback services.
