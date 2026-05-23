// NewEngine SkyDome cloud layer helper (HLSL source contract)
// Cloud sources are packed in textures/fps/skydome.ytd and selected as
// textures/fps/skydome.ytd@cloud_<profile>__<entry>.
#ifndef NEWENGINE_SKYDOME_CLOUD_LAYER_HLSL
#define NEWENGINE_SKYDOME_CLOUD_LAYER_HLSL

float3 NewEngineEvaluateCloudLayer(
    Texture2D cloud_density,
    Texture2D cloud_normal,
    SamplerState cloud_sampler,
    float2 uv,
    float3 atmosphere,
    float3 sun_color,
    float sun_elevation,
    float coverage,
    float normal_strength)
{
    float4 density_sample = cloud_density.Sample(cloud_sampler, uv);
    float3 n = cloud_normal.Sample(cloud_sampler, uv).xyz * 2.0 - 1.0;
    n.z = sqrt(saturate(1.0 - dot(n.xy, n.xy)));

    float day = smoothstep(-0.10, 0.22, sun_elevation);
    float density = saturate(max(max(density_sample.r, density_sample.g), density_sample.b) * max(coverage, 0.0));
    float edge = smoothstep(0.15, 0.95, density);
    float forward = saturate(dot(normalize(n), normalize(float3(0.35, 0.72, 0.42))) * 0.5 + 0.5);

    float3 night_cloud = float3(0.055, 0.065, 0.095);
    float3 day_cloud = lerp(float3(0.78, 0.82, 0.88), sun_color, 0.22 + forward * normal_strength * 0.28);
    float3 cloud = lerp(night_cloud, day_cloud, day) * edge;

    return lerp(atmosphere, max(atmosphere + cloud * 0.62, float3(0.0, 0.0, 0.0)), density);
}

#endif
