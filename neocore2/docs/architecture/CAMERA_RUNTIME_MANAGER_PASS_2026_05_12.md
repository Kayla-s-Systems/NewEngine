# Camera Runtime Manager Pass

The camera runtime manager centralizes runtime camera selection and keeps gameplay camera logic outside renderer ownership.

Responsibilities:

- track the active camera director;
- provide stable camera snapshots for render extraction;
- keep presentation state deterministic;
- avoid direct renderer ownership of gameplay camera policy.
