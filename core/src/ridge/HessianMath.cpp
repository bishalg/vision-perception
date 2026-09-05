#include "curv/ridge/HessianMath.hpp"
#include <algorithm>
#include <cmath>

namespace CurvEngine::ridge {

HessianEigenResult computeHessianEigen2x2(float rxx, float rxy, float ryy) noexcept {
    HessianEigenResult res;

    const float trace = rxx + ryy;
    const float diff = rxx - ryy;
    const float disc = std::sqrt(diff * diff + 4.0f * rxy * rxy);

    const float mu1 = 0.5f * (trace + disc);
    const float mu2 = 0.5f * (trace - disc);

    if (std::abs(mu1) >= std::abs(mu2)) {
        res.lambda1 = mu1;
        res.lambda2 = mu2;
    } else {
        res.lambda1 = mu2;
        res.lambda2 = mu1;
    }

    // Solve for eigenvector n of lambda1:
    // (rxx - lambda1)*nx + rxy*ny = 0
    // rxy*nx + (ryy - lambda1)*ny = 0
    const float v1x = rxy;
    const float v1y = res.lambda1 - rxx;
    const float len1_sq = v1x * v1x + v1y * v1y;

    const float v2x = res.lambda1 - ryy;
    const float v2y = rxy;
    const float len2_sq = v2x * v2x + v2y * v2y;

    float vx = 0.0f;
    float vy = 0.0f;
    float max_len_sq = 0.0f;

    if (len1_sq >= len2_sq) {
        vx = v1x;
        vy = v1y;
        max_len_sq = len1_sq;
    } else {
        vx = v2x;
        vy = v2y;
        max_len_sq = len2_sq;
    }

    if (max_len_sq > 1e-12f) {
        const float inv_norm = 1.0f / std::sqrt(max_len_sq);
        res.nx = vx * inv_norm;
        res.ny = vy * inv_norm;
    } else {
        res.nx = 1.0f;
        res.ny = 0.0f;
    }

    return res;
}

} // namespace CurvEngine::ridge
