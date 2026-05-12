# Render Plugin Contract Cleanup

The render backend is now discovered as a normal plugin service provider.
Standalone render-backend ABI probing has been removed from plugin discovery; renderer modules must export plugin metadata and provide the `render.api` service capability.

Current boundary:

- service id: `render.api`
- methods: `info_json`, `invoke_json`
- request envelope: `RenderServiceRequest`
- response envelope: `RenderServiceResponse`
- frame packet: `RenderFrameEnvelope`
