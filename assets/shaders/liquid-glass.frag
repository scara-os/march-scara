#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;

#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

uniform vec2 u_resolution;   // Ukuran panel kaca (px)
uniform vec4 u_panel_uv;     // (uv_x0, uv_y0, uv_w, uv_h) posisi panel di dalam tekstur blur
uniform float u_radius;      // Corner radius (px), misal 21.0 untuk Dock
uniform float u_refraction;  // Kekuatan pembiasan Liquid Glass dari init.lua
uniform vec4 u_tint;         // Warna tint kaca halus

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

float roundedBoxSDF(vec2 centerPos, vec2 size, float radius) {
    return length(max(abs(centerPos) - size + radius, 0.0)) - radius;
}

void main() {
    vec2 localUV = clamp((v_coords - u_panel_uv.xy) / max(u_panel_uv.zw, vec2(0.0001)), 0.0, 1.0);
    vec2 pixelPos = localUV * u_resolution;
    vec2 halfSize = u_resolution * 0.5;
    vec2 centerPos = pixelPos - halfSize;

    float dist = roundedBoxSDF(centerPos, halfSize, u_radius);
    if (dist > 0.5) {
        discard;
    }

    float edgeAlpha = 1.0 - smoothstep(-1.0, 0.5, dist);

    // Pembiasan optik di tepi kaca (Liquid Glass lens edge)
    float edgeFactor = smoothstep(-26.0, 0.0, dist);
    vec2 dir = normalize(centerPos + vec2(0.0001));
    vec2 uvOffset = dir * (edgeFactor * u_refraction);

    vec2 uvR = clamp(v_coords - uvOffset * 1.12, u_panel_uv.xy + 0.001, u_panel_uv.xy + u_panel_uv.zw - 0.001);
    vec2 uvG = clamp(v_coords - uvOffset,        u_panel_uv.xy + 0.001, u_panel_uv.xy + u_panel_uv.zw - 0.001);
    vec2 uvB = clamp(v_coords - uvOffset * 0.88, u_panel_uv.xy + 0.001, u_panel_uv.xy + u_panel_uv.zw - 0.001);

    float r = texture2D(tex, uvR).r;
    float g = texture2D(tex, uvG).g;
    float b = texture2D(tex, uvB).b;
    vec3 glassColor = vec3(r, g, b);

    // Campurkan dengan tint kaca halus
    vec3 finalRgb = mix(glassColor, u_tint.rgb, u_tint.a);

    // Kilau specular tipis di tepi lengkungan kaca
    float rim = smoothstep(-2.0, -0.3, dist) * (1.0 - smoothstep(-0.3, 0.5, dist));
    float topShine = clamp(1.15 - localUV.y * 1.2, 0.25, 1.0);
    finalRgb += vec3(0.88, 0.93, 1.0) * rim * 0.34 * topShine;

    gl_FragColor = vec4(finalRgb, 1.0) * edgeAlpha * alpha;
}
