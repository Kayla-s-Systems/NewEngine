# newengine-service-kit

Common helpers for `ServiceV1` implementations that expose JSON-control methods and engine-runtime provider-route candidates.

This crate is intentionally generic: domain crates still own DTOs, method constants and typed adapters. The kit only removes repeated `ServiceV1` boilerplate, JSON payload handling and engine-runtime provider-route registration ceremony.
