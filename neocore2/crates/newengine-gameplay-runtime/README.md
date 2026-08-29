# newengine-gameplay-runtime

Compatibility composition facade only.

The implementation has been split into independent providers: `newengine-tags-runtime`, `newengine-tasks-runtime`, `newengine-animation-foundation-runtime`, `newengine-navigation-runtime`, and `newengine-ai-runtime`. New composition code must depend on those leaf crates directly.
