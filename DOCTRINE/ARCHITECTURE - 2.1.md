# Architecture Doctrine 2.1

This note refines the host/plugin target into a gateway-first runtime model.

The engine accepts provider descriptions, validates the declared service kind, and builds an active route table. It does not depend on DLL names or hard-coded provider ids.

Runtime systems should depend on typed APIs and engine gateways, not concrete provider packages.
