# newengine-tasks-api

Stable DTO contract for `engine.tasks`.

Tasks are declarative gameplay requests shared by AI, animation, navigation,
mission logic, interaction, scripting and UI/debug tools. Consumers emit task
requests; runtime apply stages decide how and when world state mutates.
