# newengine-material-domain-api

Host-side contract for render material-domain providers.

This crate is intentionally not a renderer backend API. Renderer backends still
own native GPU objects behind `render.api`; material-domain providers create the
engine-side pipeline bundles that draw-list and pass code consume.
