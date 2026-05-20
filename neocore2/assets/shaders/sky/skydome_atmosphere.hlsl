// NewEngine SkyDome atmosphere helper (HLSL source contract)
// Provider-owned render backends may compile this through the shader registry once
// the sky pass is split from the generic lit primitive path.
#ifndef NEWENGINE_SKYDOME_ATMOSPHERE_HLSL
#define NEWENGINE_SKYDOME_ATMOSPHERE_HLSL

struct NewEngineSkyFrame
{
    float3 camera_ws;
    float3 sun_to_camera_dir;   // normalized direction from camera toward visible solar disk
    float3 sun_color;
    float  sun_intensity;
    float  time_of_day_hours;
    float  cloud_coverage;
    float  horizon_fog;
    float  reserved0;
};

float ne_saturate(float v) { return saturate(v); }
float3 ne_lerp3(float3 a, float3 b, float t) { return lerp(a, b, saturate(t)); }

float3 NewEngineEvaluateSkyAtmosphere(float3 view_dir, NewEngineSkyFrame sky)
{
    view_dir = normalize(view_dir);
    float sun_elevation = sky.sun_to_camera_dir.y;
    float day = smoothstep(-0.10, 0.22, sun_elevation);
    float horizon = pow(saturate(1.0 - abs(view_dir.y)), 1.65);

    float3 night_zenith = float3(0.008, 0.014, 0.035);
    float3 day_zenith = float3(0.16, 0.36, 0.78);
    float3 zenith = lerp(night_zenith, day_zenith, day);

    float sunset_gate = smoothstep(-0.04, 0.18, sun_elevation) * (1.0 - smoothstep(0.16, 0.52, sun_elevation));
    float3 day_horizon = lerp(float3(0.54, 0.72, 0.96), float3(1.0, 0.50, 0.23), sunset_gate);
    float3 horizon_color = lerp(float3(0.018, 0.022, 0.050), day_horizon, day);

    float sun_dot = saturate(dot(view_dir, normalize(sky.sun_to_camera_dir)));
    float disk = pow(sun_dot, 4096.0) * 3.8;
    float halo = pow(sun_dot, 64.0) * 0.18 + pow(sun_dot, 12.0) * 0.025;

    return max(lerp(zenith, horizon_color, horizon) + sky.sun_color * sky.sun_intensity * day * (disk + halo), float3(0.0, 0.0, 0.0));
}

#endif
