import Foundation
import CoreGraphics

/// Per-user affine correction from raw ARKit `lookAtPoint` raycast estimates to true
/// screen-normalized coordinates.
///
/// Raw TrueDepth gaze (`ScreenRaycaster.projectGazeToScreen`) is an uncalibrated
/// anatomical estimate: measured error is 3-6 cm on the glass, which is most of a
/// phone's 7.1 cm width. A single 2D affine map (6 degrees of freedom, fit from the
/// 5-point calibration in `BubbleCalibrationView`) absorbs the dominant per-user
/// error sources — eye-to-lookAtPoint offset bias, camera/display geometry
/// mismatch across device models — without needing a full homography (which is
/// more overfit-prone with only 5 points).
///
/// `x' = a*x + b*y + c`
/// `y' = d*x + e*y + f`
public struct GazeCalibrationProfile: Codable, Sendable, Equatable {
    public var a: Double
    public var b: Double
    public var c: Double
    public var d: Double
    public var e: Double
    public var f: Double
    public var sampleCount: Int
    public var createdAt: Date

    /// Passthrough transform — used before a user has ever calibrated.
    public static let identity = GazeCalibrationProfile(
        a: 1, b: 0, c: 0, d: 0, e: 1, f: 0, sampleCount: 0, createdAt: .distantPast
    )

    public var isCalibrated: Bool { sampleCount > 0 }

    public init(a: Double, b: Double, c: Double, d: Double, e: Double, f: Double, sampleCount: Int, createdAt: Date) {
        self.a = a; self.b = b; self.c = c; self.d = d; self.e = e; self.f = f
        self.sampleCount = sampleCount
        self.createdAt = createdAt
    }

    /// Applies the affine map and clamps to the valid screen-normalized range.
    public func apply(to raw: CGPoint) -> CGPoint {
        let x = a * Double(raw.x) + b * Double(raw.y) + c
        let y = d * Double(raw.x) + e * Double(raw.y) + f
        return CGPoint(x: min(max(x, 0.0), 1.0), y: min(max(y, 0.0), 1.0))
    }

    /// Fits the affine map by least squares from paired (raw gaze, true target)
    /// points collected during calibration. Needs at least 3 non-collinear points;
    /// the 5-point center+corners layout in `BubbleCalibrationView` is well over
    /// that minimum and well-conditioned (spans both axes independently).
    ///
    /// x and y are solved independently: each is a plane fit `v = p*x_raw + q*y_raw + r`
    /// via the normal equations, solved with 3x3 Gaussian elimination (no external
    /// linear-algebra dependency needed for a fixed, tiny system).
    public static func fit(rawPoints: [CGPoint], targetPoints: [CGPoint]) -> GazeCalibrationProfile? {
        guard rawPoints.count == targetPoints.count, rawPoints.count >= 3 else { return nil }
        let n = rawPoints.count

        // Build the shared normal-equation matrix once (independent of which axis
        // we're solving for) plus one right-hand side per axis.
        var sxx = 0.0, sxy = 0.0, sx = 0.0, syy = 0.0, sy = 0.0
        var sxt = 0.0, syt = 0.0, st = 0.0  // for target-x
        var sxu = 0.0, syu = 0.0, su = 0.0  // for target-y

        for i in 0..<n {
            let x = Double(rawPoints[i].x), y = Double(rawPoints[i].y)
            let tx = Double(targetPoints[i].x), ty = Double(targetPoints[i].y)
            sxx += x * x; sxy += x * y; sx += x
            syy += y * y; sy += y
            sxt += x * tx; syt += y * tx; st += tx
            sxu += x * ty; syu += y * ty; su += ty
        }
        let sn = Double(n)

        // Solve [sxx sxy sx; sxy syy sy; sx sy sn] * [p;q;r] = [sxt;syt;st] for x-axis
        // and the same matrix with [sxu;syu;su] for y-axis.
        let matrix: [[Double]] = [
            [sxx, sxy, sx],
            [sxy, syy, sy],
            [sx, sy, sn]
        ]
        guard let xCoeffs = solve3x3(matrix, [sxt, syt, st]),
              let yCoeffs = solve3x3(matrix, [sxu, syu, su]) else { return nil }

        return GazeCalibrationProfile(
            a: xCoeffs[0], b: xCoeffs[1], c: xCoeffs[2],
            d: yCoeffs[0], e: yCoeffs[1], f: yCoeffs[2],
            sampleCount: n,
            createdAt: Date()
        )
    }

    /// Solves a 3x3 linear system via Gaussian elimination with partial pivoting.
    /// Returns nil if the matrix is singular (e.g. all calibration points identical).
    private static func solve3x3(_ matrix: [[Double]], _ rhs: [Double]) -> [Double]? {
        var m = matrix
        var b = rhs
        let n = 3

        for col in 0..<n {
            var pivotRow = col
            var maxAbs = abs(m[col][col])
            for row in (col + 1)..<n where abs(m[row][col]) > maxAbs {
                maxAbs = abs(m[row][col])
                pivotRow = row
            }
            guard maxAbs > 1e-9 else { return nil }
            if pivotRow != col {
                m.swapAt(col, pivotRow)
                b.swapAt(col, pivotRow)
            }
            for row in (col + 1)..<n {
                let factor = m[row][col] / m[col][col]
                for k in col..<n { m[row][k] -= factor * m[col][k] }
                b[row] -= factor * b[col]
            }
        }

        var x = [Double](repeating: 0, count: n)
        for row in stride(from: n - 1, through: 0, by: -1) {
            var sum = b[row]
            for k in (row + 1)..<n { sum -= m[row][k] * x[k] }
            x[row] = sum / m[row][row]
        }
        return x
    }
}

/// UserDefaults-backed persistence — mirrors the pattern already used for care-plan
/// state (`CarePlanModels.swift`). Single active profile per device: recalibrate
/// before screening a different child.
public enum GazeCalibrationStore {
    private static let key = "CurvAsdGazeKit.calibrationProfile.v1"

    public static func load() -> GazeCalibrationProfile {
        guard let data = UserDefaults.standard.data(forKey: key),
              let profile = try? JSONDecoder().decode(GazeCalibrationProfile.self, from: data)
        else { return .identity }
        return profile
    }

    public static func save(_ profile: GazeCalibrationProfile) {
        guard let data = try? JSONEncoder().encode(profile) else { return }
        UserDefaults.standard.set(data, forKey: key)
    }

    public static func reset() {
        UserDefaults.standard.removeObject(forKey: key)
    }
}
