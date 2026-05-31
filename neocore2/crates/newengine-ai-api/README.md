# newengine-ai-api

Stable DTO contract for `engine.ai`.

AI observes frame DTOs and emits intent DTOs. It never receives `&mut World`,
raw ECS storage or native entity ids, and it never mutates ECS/world directly.
Runtime apply stages translate intents into domain commands.
