#version 330

in vec3 v_Norm;
in vec2 v_UvPos;
in vec2 v_UvSize;
in vec2 v_UvOffset;
in float occl;
in float v_LightLevel;

out vec4 ColorBuffer;

uniform sampler2D TextureAtlas;

const vec3 SUN_DIRECTION = normalize(vec3(0, 1, 0.5));
const float SUN_FRACTION = 0.3;

void main() {
    float lightFactor = v_LightLevel / 15.0;
    if(abs(v_LightLevel - 15.0) < 1e-5) {
        ColorBuffer = vec4(1.0, 0.0, 0.0, 1.0);
    } else {
        vec2 actualPosition = v_UvPos + mod(v_UvOffset, v_UvSize);
        ColorBuffer = lightFactor * texture(TextureAtlas, actualPosition) * occl * vec4(1.0, 1.0, 1.0, 1.0) * (1.0 - SUN_FRACTION + SUN_FRACTION * abs(dot(v_Norm, SUN_DIRECTION)));
    }
}
