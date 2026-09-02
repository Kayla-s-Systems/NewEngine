# newengine-startup-intro

Platform-neutral, data-driven startup intro orchestration for North Star.

This crate owns the descriptor schema, validation, ordered sequence resolution and the presenter port. It deliberately owns no window system, decoder, codec stack or OS media API. Concrete presenters are upper-layer providers registered through `install_startup_intro_presenter`, following the same UNIX-like port/provider pattern used by PreStart.

Adding or reordering logos changes only `intro.toml`; adding another operating-system/media implementation adds a provider crate without changing this contract.
