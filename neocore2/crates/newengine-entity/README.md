# newengine-entity

Foundation crate that defines the **EntityId** type for the whole NewEngine ecosystem.

## Why

Entity identity must be **independent** from a specific ECS storage implementation.
This keeps layer boundaries clean and enables multiple ECS backends while preserving
stable cross-crate contracts.
