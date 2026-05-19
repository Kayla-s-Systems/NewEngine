# CoreEngine Documentation

This folder contains architecture notes, runtime reports, codec specifications, and provider-system guides for CoreEngine / NewEngine.

Primary entry points:

- `CoreEngine_Documentation/01_ARCHITECTURE_OVERVIEW.md`
- `CoreEngine_Documentation/03_PROVIDER_ADAPTER_SYSTEM_GUIDE.md`
- `CoreEngine_Documentation/15_GATEWAY_SERVICE_LAYER.md`
- `CoreEngine_Documentation/12_NEYTD_CODEC_SPEC.md`
- `CoreEngine_Documentation/13_NEPAK_CODEC_SPEC.md`
- `INPUT_SYSTEMS_RUNTIME.md`

The current architecture is gateway-first: consumers use engine-owned facade ids, and the host resolves those ids to provider-owned plugin services through descriptor metadata.
