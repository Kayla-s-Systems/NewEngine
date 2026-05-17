# Engine Structure Doctrine

Move host adapters and provider-specific implementation out of reusable engine-runtime code.

The engine should remain clean and extensible:

```text
engine core -> stable contracts and gateway routing
provider plugins -> implementation details
runtime adapters -> typed wrappers over selected services
```
