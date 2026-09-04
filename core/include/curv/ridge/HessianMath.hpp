#pragma once

namespace CurvEngine::ridge {

/**
 * @brief Eigendecomposition result for a 2x2 symmetric Hessian matrix.
 */
struct HessianEigenResult {
    float lambda1{0.0f}; ///< Dominant eigenvalue (|lambda1| >= |lambda2|)
    float lambda2{0.0f}; ///< Secondary eigenvalue
    float nx{0.0f};      ///< Unit normal X (eigenvector for lambda1)
    float ny{0.0f};      ///< Unit normal Y (eigenvector for lambda1)
};

/**
 * @brief Computes closed-form analytical eigenvalue decomposition of a 2x2 symmetric Hessian matrix.
 *
 * Matrix:
 *   [ rxx  rxy ]
 *   [ rxy  ryy ]
 */
[[nodiscard]] HessianEigenResult computeHessianEigen2x2(float rxx, float rxy, float ryy) noexcept;

/**
 * @brief Computes sub-pixel Taylor expansion zero-crossing offset t along the normal direction.
 *
 * @param rx First partial derivative in X
 * @param ry First partial derivative in Y
 * @param nx Normal vector X
 * @param ny Normal vector Y
 * @param lambda1 Dominant second derivative (curvature along normal)
 * @return float Sub-pixel offset t (valid ridge if |t| <= 0.5)
 */
[[nodiscard]] inline float computeSubpixelOffset(float rx, float ry, float nx, float ny, float lambda1) noexcept {
    if (lambda1 == 0.0f) return 100.0f; // Invalid
    const float f_prime = rx * nx + ry * ny;
    return -f_prime / lambda1;
}

} // namespace CurvEngine::ridge
