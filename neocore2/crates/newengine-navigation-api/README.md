# newengine-navigation-api

Stable DTO contract for `engine.navigation`.

Navigation providers answer queries and produce path DTOs. They do not mutate
ECS/world directly; movement is requested through AI/task/world apply intents.
