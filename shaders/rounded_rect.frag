#version 330 core

out vec4 color;
in vec2 fragCoord;

uniform float edgeSoftness;
uniform vec2 size;
uniform vec4 bgColor;
uniform vec4 borderColor;
uniform float borderThickness;
// top-left top-right bottom-left bottom-right
uniform vec4 borderRadius; 

float box(vec2 position, vec2 halfSize, vec4 cornerRadius) {
    float corner = cornerRadius.x;
    if (position.x > 0.0 && position.y > 0.0) {
        corner = cornerRadius.w;
    }
    if (position.x > 0.0 && position.y < 0.0) {
        corner = cornerRadius.y;
    }
    if (position.x < 0.0 && position.y > 0.0) {
        corner = cornerRadius.z;
    }
    position = abs(position) - halfSize + corner;
    return length(max(position, 0.0)) + min(max(position.x, position.y), 0.0) - corner;
}

// Quad needs to be a little bit bigger
void main() {
    color = vec4(size, 0.0, 1.0);
    vec2 center = size / 2.0;

    float distance = box(
        (fragCoord * (size+edgeSoftness*2.0)/size - 0.5) * size - edgeSoftness, 
        size / 2.0, 
        borderRadius
    );
    float smoothedAlpha = 1.0 - smoothstep(0.0, edgeSoftness, distance);
    float borderAlpha = 1.0 - smoothstep(borderThickness - 1.0, borderThickness, abs(distance));
    vec4 xcolor = mix(bgColor, borderColor, borderAlpha);

    color = vec4(xcolor.rgb, min(smoothedAlpha, xcolor.a));
}
