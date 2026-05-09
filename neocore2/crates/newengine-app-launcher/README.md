# newengine-app-launcher

Shared bootstrap for NewEngine binaries.

This crate exists to keep app binaries declarative: editor and game-ready launchers describe only their identity and environment toggles; config loading, logging, plugin bootstrap, asset roots, platform runtime detection and UI runtime handoff are centralized here.
