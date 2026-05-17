# Controller Doctrine

Entity controllers should produce intent commands rather than mutating world storage directly.

Recommended frame shape:

```text
input snapshot -> controller update -> intent buffer -> deterministic apply stage
```

This preserves single-writer ownership, improves replay support, and keeps gameplay policy separate from storage implementation.
