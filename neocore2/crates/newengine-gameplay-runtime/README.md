# newengine-gameplay-runtime

Core-owned baseline providers for the gameplay foundation domains:

- `engine.tags`
- `engine.tasks`
- `engine.animation`
- `engine.navigation`
- `engine.ai`

These providers are compiled with the runtime profile so gameplay foundations are
available without a mandatory external plugin. They still register through the
same gateway/capability route model, so external providers can replace them.
AI observes frame DTOs and returns intent DTOs; runtime apply stages own all
world/entity/ECS mutation.
