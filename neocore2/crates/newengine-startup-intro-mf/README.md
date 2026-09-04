# newengine-startup-intro-mf

Windows presenter provider for `newengine-startup-intro`.

The provider presents every enabled sequence entry inside the already-created game HWND; it never creates a second splash window. Video frames are decoded through Media Foundation Source Reader, converted to RGB32, aspect-fitted, and copied into the target HWND through a reusable GDI frame surface. MFPlay is used only for synchronized audio when an entry has non-zero volume.

Descriptor parsing, validation, ordering, skip policy, and per-entry timeouts remain owned by `newengine-startup-intro`. Install the provider from a Windows host composition with `newengine_startup_intro_mf::install()`.
