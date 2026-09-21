import SwiftUI
import CurvCameraOverlay
import CurvPalmOverlay

// MARK: - Fingertip Scan Mode Overlay
//
// All UI components specific to the `.fingertipScan` mode live here.
// Nothing in this file is used by Still Photo, 3D Orbit, or Close-Up modes.

// MARK: - Reticle (inside CameraOverlayHost reticle box)

/// Center macro target reticle for fingertip epidermal ridge scanning.
/// Hosts a `touchid` icon, outer dashed ring, and an animated dwell progress
/// ring that fills from 0→1 during the 1.2s hold-steady gate.
struct FingertipMacroReticleView: View {
    let isCapturing: Bool
    var holdProgress: Float = 0.0

    var body: some View {
        GeometryReader { proxy in
            let center = CGPoint(x: proxy.size.width / 2, y: proxy.size.height / 2)
            let diameter = min(proxy.size.width, proxy.size.height) * 0.46

            ZStack {
                // Outer dashed targeting ring
                Circle()
                    .stroke(
                        Color.white.opacity(0.35),
                        style: StrokeStyle(lineWidth: 1.5, dash: [6, 4])
                    )
                    .frame(width: diameter * 1.15, height: diameter * 1.15)

                // Background track circle
                Circle()
                    .stroke(Color.white.opacity(0.20), lineWidth: 3.0)
                    .frame(width: diameter, height: diameter)

                // Animated dwell progress ring
                if holdProgress > 0.01 {
                    Circle()
                        .trim(from: 0.0, to: CGFloat(min(1.0, max(0.0, holdProgress))))
                        .stroke(
                            OverlayPalette.emerald,
                            style: StrokeStyle(lineWidth: 4.0, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                        .frame(width: diameter, height: diameter)
                        .animation(.linear(duration: 0.1), value: holdProgress)
                }

                // Main glass alignment circle
                Circle()
                    .stroke(
                        isCapturing ? OverlayPalette.emerald :
                            (holdProgress > 0 ? OverlayPalette.emerald.opacity(0.6) :
                                Color.white.opacity(0.85)),
                        lineWidth: isCapturing ? 3.0 : 2.0
                    )
                    .frame(width: diameter, height: diameter)
                    .shadow(
                        color: isCapturing ? OverlayPalette.emerald.opacity(0.8) :
                            Color.white.opacity(0.2),
                        radius: 8
                    )

                // Fingerprint icon — turns emerald when hold progress > 80%
                Image(systemName: "touchid")
                    .font(.system(size: diameter * 0.42, weight: .light))
                    .foregroundColor(
                        isCapturing || holdProgress > 0.8 ?
                            OverlayPalette.emerald : Color.white.opacity(0.25)
                    )

                // Four corner crosshairs
                ForEach([0, 90, 180, 270], id: \.self) { deg in
                    Rectangle()
                        .fill(Color.white.opacity(0.7))
                        .frame(width: 2, height: 12)
                        .offset(y: -(diameter / 2 + 8))
                        .rotationEffect(.degrees(Double(deg)))
                }
            }
            .position(center)
        }
    }
}

// MARK: - Guidance Overlay (ZStack sibling over camera feed)

/// HUD coaching chip displaying the target digit name and coaching below mode switcher.
/// Shows "HOLD STEADY XX%" during dwell accumulation.
struct FingertipGuidanceOverlayView: View {
    let session: FingertipScanSession
    var state: FingertipGuidanceState = .searching
    var holdProgress: Float = 0.0
    let isCapturing: Bool

    private var nextDigit: FingertipDigit? {
        FingertipDigit.allCases.first { !session.scannedDigits.contains($0) }
    }

    var body: some View {
        if let digit = nextDigit {
            VStack(spacing: 5) {
                HStack(spacing: 8) {
                    Image(systemName: "hand.point.up.fill")
                        .foregroundColor(OverlayPalette.emerald)
                    Text("Target: \(digit.displayName) Pad")
                        .font(.system(size: 14, weight: .semibold, design: .rounded))
                        .foregroundColor(.white)
                }

                if holdProgress > 0.05 {
                    HStack(spacing: 6) {
                        Text("HOLD STEADY")
                            .font(.system(size: 11, weight: .bold, design: .rounded))
                            .foregroundColor(OverlayPalette.emerald)
                        Text("\(Int(holdProgress * 100))%")
                            .font(.system(size: 11, weight: .semibold, design: .monospaced))
                            .foregroundColor(.white)
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(
                Capsule()
                    .fill(Color.black.opacity(0.75))
                    .overlay(
                        Capsule().stroke(
                            holdProgress > 0 ?
                                OverlayPalette.emerald :
                                OverlayPalette.emerald.opacity(0.5),
                            lineWidth: holdProgress > 0 ? 1.5 : 1.0
                        )
                    )
            )
            .shadow(color: .black.opacity(0.4), radius: 8, y: 3)
            .allowsHitTesting(false)
        }
    }
}

// MARK: - Bottom Card

/// Bottom control card for fingertip pattern scanning.
/// Shows 5 digit status pills, manual-capture button, review button, and reading button.
struct FingertipScanBottomCard: View {
    let session: FingertipScanSession
    let isCapturing: Bool
    let statusMessage: String
    let onCapture: () -> Void
    let onReset: () -> Void
    let onInspect: () -> Void
    let onViewReading: () -> Void

    private var activeDigit: FingertipDigit? {
        FingertipDigit.allCases.first { !session.scannedDigits.contains($0) }
    }

    var body: some View {
        VStack(spacing: 12) {
            // 5 Digit status indicators
            HStack(spacing: 6) {
                ForEach(FingertipDigit.allCases, id: \.self) { digit in
                    DigitStatusPill(
                        digit: digit,
                        result: session.results.first(where: { $0.digit == digit }),
                        isActive: digit == activeDigit
                    )
                }
            }

            // Coaching instruction
            Text(statusMessage)
                .font(.system(size: 14, weight: .semibold, design: .rounded))
                .foregroundColor(.white)
                .multilineTextAlignment(.center)
                .contentTransition(.opacity)

            // Action buttons
            HStack(spacing: 10) {
                Button(action: onReset) {
                    Text("Reset")
                        .font(.system(size: 12, weight: .semibold, design: .rounded))
                        .foregroundColor(.white.opacity(0.75))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Capsule().stroke(Color.white.opacity(0.3), lineWidth: 1))
                }
                .buttonStyle(.plain)

                if let digit = activeDigit {
                    Button(action: onCapture) {
                        HStack(spacing: 6) {
                            if isCapturing {
                                ProgressView()
                                    .progressViewStyle(CircularProgressViewStyle(tint: .black))
                                    .scaleEffect(0.8)
                            } else {
                                Image(systemName: "camera.fill")
                            }
                            Text("Capture \(digit.displayName)")
                        }
                        .font(.system(size: 13, weight: .bold, design: .rounded))
                        .foregroundColor(.black)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 8)
                        .background(Capsule().fill(Color.white))
                    }
                    .buttonStyle(.plain)
                    .disabled(isCapturing)
                }

                if !session.results.isEmpty {
                    Button(action: onInspect) {
                        HStack(spacing: 4) {
                            Image(systemName: "slider.horizontal.3")
                            Text("Review")
                        }
                        .font(.system(size: 12, weight: .semibold, design: .rounded))
                        .foregroundColor(.white)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Capsule().fill(Color.white.opacity(0.18)))
                    }
                    .buttonStyle(.plain)

                    Button(action: onViewReading) {
                        HStack(spacing: 4) {
                            Image(systemName: "sparkles")
                            Text("Reading")
                        }
                        .font(.system(size: 12, weight: .bold, design: .rounded))
                        .foregroundColor(.black)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Capsule().fill(OverlayPalette.emerald))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.top, 2)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(Color.black.opacity(0.78))
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(Color.white.opacity(0.2), lineWidth: 0.8)
                )
        )
        .shadow(color: .black.opacity(0.4), radius: 12, y: 4)
    }
}

// MARK: - Digit Status Pill (private sub-view)

private struct DigitStatusPill: View {
    let digit: FingertipDigit
    let result: FingertipScanResult?
    let isActive: Bool

    private var isScanned: Bool { result != nil }

    var body: some View {
        VStack(spacing: 3) {
            HStack(spacing: 2) {
                if isScanned {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 10))
                        .foregroundColor(OverlayPalette.emerald)
                } else if isActive {
                    Circle()
                        .fill(Color.white)
                        .frame(width: 6, height: 6)
                }
                Text(digit.displayName)
                    .font(.system(size: 11,
                                  weight: isActive ? .bold : .medium,
                                  design: .rounded))
                    .foregroundColor(
                        isActive ? .white :
                            isScanned ? OverlayPalette.emerald : .white.opacity(0.5)
                    )
            }

            if let res = result {
                Text(res.pattern.rawValue.capitalized)
                    .font(.system(size: 9, weight: .semibold, design: .rounded))
                    .foregroundColor(OverlayPalette.emerald.opacity(0.9))
            } else {
                Text(isActive ? "Active" : "Pending")
                    .font(.system(size: 9, weight: .regular, design: .rounded))
                    .foregroundColor(.white.opacity(0.4))
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 6)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(
                    isActive ? Color.white.opacity(0.18) :
                        isScanned ? OverlayPalette.emerald.opacity(0.15) :
                        Color.white.opacity(0.06)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(
                            isActive ? Color.white.opacity(0.6) :
                                isScanned ? OverlayPalette.emerald.opacity(0.4) :
                                Color.clear,
                            lineWidth: 1
                        )
                )
        )
    }
}
