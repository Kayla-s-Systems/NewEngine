#version 450

layout(location = 0) in vec3 vColor;
layout(location = 1) in vec2 vPosNdc;
layout(location = 2) in vec3 vBary;

layout(location = 0) out vec4 oColor;

// Push constants: 16 bytes (std430-like alignment). Use vec4 to avoid alignment issues.
layout(push_constant) uniform Push {
    vec4 data; // x=time, y=aspect, z=unused, w=unused
} pc;

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0/3.0, 1.0/3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

void main() {
    float t = pc.data.x;
    float aspect = max(pc.data.y, 0.0001);

    // NDC -> "screen-ish" space with aspect correction
    vec2 p = vec2(vPosNdc.x * aspect, vPosNdc.y);

    float r = length(p);

    // Barycentric edges
    float e = min(min(vBary.x, vBary.y), vBary.z);
    float w = fwidth(e) * 2.0;
    float outline = 1.0 - smoothstep(0.0, w, e);

    // Inner glow stronger near edges
    float inner = smoothstep(0.0, 0.22, e);
    float glow = pow(1.0 - inner, 2.1);

    // Animated scanlines + interference
    float scan = 0.90 + 0.10 * sin(gl_FragCoord.y * 1.65 + t * 18.0);
    float bands = sin(p.y * 55.0 + sin(p.x * 14.0 + t * 2.0) * 2.2 + t * 6.0) * 0.5 + 0.5;

    // Moving rings / holo
    float rings = sin((r * 16.0 - t * 4.0 + p.x * 5.0) * 3.14159) * 0.5 + 0.5;
    float film = smoothstep(0.15, 0.95, 0.60 * bands + 0.40 * rings);

    // Neon hue drift
    float hue = fract(0.62 + 0.12 * sin(t * 0.9) + 0.10 * film + 0.08 * p.x);
    vec3 neon = hsv2rgb(vec3(hue, 0.95, 1.0));

    // Chromatic "feel" (fake CA without multiple samples)
    vec3 ca = vec3(1.0 + 0.07 * sin(t * 3.0 + p.x * 10.0),
    1.0 + 0.07 * sin(t * 3.0 + p.y * 11.0 + 2.0),
    1.0 + 0.07 * sin(t * 3.0 + r * 12.0 + 4.0));

    // Noise / grain
    float grain = (hash12(gl_FragCoord.xy + t * 120.0) - 0.5) * 0.06;

    // Base energized vertex color
    vec3 base = vColor;
    base = mix(base, neon, film * 0.40);
    base *= ca;

    // Edge emissive
    vec3 edge = neon * (0.85 + 0.35 * sin(t * 6.0 + r * 8.0));
    vec3 color = base;
    color += edge * (outline * 1.10);
    color += edge * (glow * 0.40);

    // Vignette-ish + subtle pulse
    float vig = smoothstep(1.05, 0.25, r);
    float pulse = 0.88 + 0.12 * sin(t * 2.2);
    color *= (0.78 + 0.22 * vig) * pulse;

    color *= scan;
    color += grain;

    // Slight tone shaping
    color = pow(max(color, 0.0), vec3(0.92));

    oColor = vec4(color, 1.0);
}