#version 450

layout(location = 0) in vec3 vColor;
layout(location = 1) in vec2 vPosNdc;
layout(location = 2) in vec3 vBary;

layout(location = 0) out vec4 oColor;

// Push constants: vec4(data): x=time, y=aspect
layout(push_constant) uniform Push {
    vec4 data;
} pc;

float saturate(float x) { return clamp(x, 0.0, 1.0); }
vec3  saturate(vec3 v)  { return clamp(v, 0.0, 1.0); }

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float noise2(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    float a = hash12(i);
    float b = hash12(i + vec2(1.0, 0.0));
    float c = hash12(i + vec2(0.0, 1.0));
    float d = hash12(i + vec2(1.0, 1.0));
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.55;
    mat2 m = mat2(1.6, -1.2, 1.2, 1.6);
    for (int i = 0; i < 4; ++i) {
        v += a * noise2(p);
        p = m * p;
        a *= 0.5;
    }
    return v;
}

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0/3.0, 1.0/3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

// Soft, animated edge: uses barycentric distance + noise-driven width & wobble
float softEdgeMask(vec3 bary, vec2 p, float t, out float edgeDist) {
    float e = min(min(bary.x, bary.y), bary.z);  // distance-to-edge in barycentric space
    edgeDist = e;

    // Pixel footprint
    float fw = max(fwidth(e), 1e-5);

    // "Living" wobble along the edge (stable, no sparkle)
    float n = fbm(p * 6.0 + vec2(t * 0.9, -t * 0.7));
    float wobble = (n - 0.5) * (fw * 8.0);

    // Breathing width (time-based)
    float breathe = 1.0 + 0.55 * sin(t * 2.3) + 0.25 * sin(t * 5.1);

    // Soft transition band; bigger => blurrier edge
    float width = fw * (6.0 * breathe + 10.0 * saturate(n));

    // Edge coverage: 1 near boundary, 0 deeper inside
    float edge = 1.0 - smoothstep(0.0, width, e + wobble);

    return saturate(edge);
}

void main() {
    float t = pc.data.x;
    float aspect_in = pc.data.y;
    float aspect = (aspect_in > 0.0001) ? aspect_in : 1.0;

    // Screen-ish coords from NDC
    vec2 p = vec2(vPosNdc.x * aspect, vPosNdc.y);
    vec2 uv = p * 0.5 + 0.5;

    float r = length(p);
    float ang = atan(p.y, p.x);

    // Rare glitch events
    float tearNoise = fbm(vec2(gl_FragCoord.y * 0.012, t * 2.4));
    float tear = step(0.92, tearNoise) * (tearNoise - 0.92) * 12.0;

    // Tile-based glitch blocks
    vec2 tile = floor(gl_FragCoord.xy / vec2(24.0, 18.0));
    float tileRnd = hash12(tile + floor(t * 10.0));
    float tileOn = step(0.74, tileRnd) * step(0.55, tearNoise);

    // Horizontal tearing band shift
    float band = smoothstep(0.0, 0.8, fbm(vec2(uv.y * 10.0, t * 3.0)));
    float bandSel = step(0.82, band);
    float hShift = (hash12(vec2(floor(uv.y * 140.0), floor(t * 18.0))) - 0.5)
        * (0.02 + 0.12 * tear) * bandSel;

    // Micro jitter
    float micro = (hash12(gl_FragCoord.xy + t * 180.0) - 0.5);
    float jitter = micro * (0.006 + 0.040 * tear + 0.025 * tileOn);

    vec2 uvD = uv + vec2(hShift + jitter, 0.0);

    // Neon palette (CP-ish: cyan/magenta drift)
    float hue = fract(0.58 + 0.06 * sin(t * 0.75) + 0.04 * sin(ang * 2.0) + 0.05 * fbm(p * 2.5 + t * 0.2));
    vec3 neon = hsv2rgb(vec3(hue, 0.95, 1.0));
    vec3 neonAlt = hsv2rgb(vec3(fract(hue + 0.18), 0.95, 1.0));

    // Base gradient from your vertex color + holo modulation
    float warpField = fbm((uvD - 0.5) * 7.0 + vec2(t * 0.6, -t * 0.4));
    vec3 base = mix(vColor, neon, 0.35 + 0.25 * warpField);

    // Living soft edge
    float edgeDist;
    float edge = softEdgeMask(vBary, p, t, edgeDist);

    // Additional wide haze around edge (so it “bleeds”)
    float fw = max(fwidth(edgeDist), 1e-5);
    float haze = 1.0 - smoothstep(0.0, fw * 22.0, edgeDist);
    haze = pow(saturate(haze), 2.0);

    // Scanlines + grain
    float scan = sin(gl_FragCoord.y * 1.8 + t * 28.0) * 0.5 + 0.5;
    scan = 0.92 + 0.08 * smoothstep(0.20, 1.0, scan);

    float grain = (hash12(gl_FragCoord.xy + t * 250.0) - 0.5) * (0.04 + 0.14 * tear);

    // Digital bars
    float bars = sin((uvD.x * 120.0) + (t * 8.0) + floor(uvD.y * 18.0) * 0.7) * 0.5 + 0.5;
    bars = pow(saturate(bars), 9.0) * (0.08 + 0.55 * tear);
    vec3 barCol = vec3(bars, bars * 0.25, bars * 1.15);

    // Core energy + rings (reactor feel)
    float pulse = 0.72 + 0.28 * sin(t * 2.6);
    float core = smoothstep(0.62, 0.00, r);
    core = pow(core, 1.25);

    float ringPhase = r * 30.0 - t * 7.5 + sin(ang * 5.0 + t * 0.8) * 0.35;
    float rings = sin(ringPhase * 3.14159265) * 0.5 + 0.5;
    rings = pow(saturate(rings), 7.0);
    rings *= smoothstep(1.10, 0.05, r);

    // Edge-driven emissive (the important part)
    vec3 edgeGlow = mix(neon, neonAlt, 0.5 + 0.5 * sin(t * 1.7 + ang * 2.0));
    vec3 emissive = vec3(0.0);
    emissive += edgeGlow * edge * (2.0 + 3.0 * pulse);       // crisp neon edge
    emissive += edgeGlow * haze * (0.9 + 1.8 * pulse);       // soft bleed / haze
    emissive += edgeGlow * core * (0.8 + 1.6 * pulse);       // inner energy
    emissive += edgeGlow * rings * (1.6 + 2.2 * pulse);      // rings

    // Block tint overlay during glitch tiles
    vec3 blockTint = mix(vec3(0.0, 1.0, 0.9), vec3(1.0, 0.0, 1.0), tileRnd);
    float blockAmp = tileOn * (0.25 + 0.85 * tear);

    // Fake chroma split (component modulation) stronger near edge + glitch
    float ca = (0.002 + 0.018 * tear + 0.010 * tileOn) * smoothstep(0.05, 0.95, r) * (0.35 + 0.65 * haze);
    vec3 caModR = vec3(1.0 + ca * 3.0, 1.0, 1.0);
    vec3 caModG = vec3(1.0, 1.0 + ca * 1.5, 1.0);
    vec3 caModB = vec3(1.0, 1.0, 1.0 + ca * 2.4);

    // Compose
    vec3 col = base + emissive + barCol;

    // Mix in block glitch coloration
    col = mix(col, blockTint, blockAmp);

    // Scanlines + grain
    col *= scan;
    col += grain;

    // Apply chroma modulation
    vec3 cR = col * caModR;
    vec3 cG = col * caModG;
    vec3 cB = col * caModB;
    col = vec3(cR.r, cG.g, cB.b);

    // Vignette to keep it cinematic
    float vig = smoothstep(1.12, 0.22, r);
    col *= (0.72 + 0.28 * vig);

    // Glitch flash
    float flash = tear * (0.55 + 0.45 * (0.5 + 0.5 * sin(t * 18.0)));
    col *= (1.0 + flash);

    // Highlight lift (pseudo-bloom without extra taps)
    float luma = dot(col, vec3(0.2126, 0.7152, 0.0722));
    float bloom = max(luma - 0.55, 0.0);
    bloom = bloom * bloom * 1.5;
    col += edgeGlow * bloom * 1.05;

    // Final shaping
    col = pow(saturate(col), vec3(0.92));
    col = saturate(col * 1.10);

    oColor = vec4(col, 1.0);
}