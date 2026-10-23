import SwiftUI
import CurvAsdGazeKit

#if canImport(ARKit) && canImport(SceneKit) && !os(macOS)
import ARKit
import SceneKit

/// Native SceneKit + ARKit view rendering the live front camera video feed
/// with real-time 3D facial wireframe mesh tracking overlaid on the face.
public struct ARFaceSceneView: UIViewRepresentable {
    public let session: ARSession?

    public init(session: ARSession?) {
        self.session = session
    }

    public func makeUIView(context: Context) -> ARSCNView {
        let scnView = ARSCNView(frame: .zero)
        scnView.automaticallyUpdatesLighting = true
        scnView.rendersCameraGrain = false
        scnView.backgroundColor = .black
        if let session {
            scnView.session = session
        }
        scnView.delegate = context.coordinator
        return scnView
    }

    public func updateUIView(_ uiView: ARSCNView, context: Context) {
        if let session, uiView.session !== session {
            uiView.session = session
        }
    }

    public static func dismantleUIView(_ uiView: ARSCNView, coordinator: Coordinator) {
        uiView.delegate = nil
        coordinator.faceGeometry = nil
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    public class Coordinator: NSObject, ARSCNViewDelegate {
        var faceGeometry: ARSCNFaceGeometry?

        public func renderer(_ renderer: SCNSceneRenderer, nodeFor anchor: ARAnchor) -> SCNNode? {
            guard let device = renderer.device, let faceAnchor = anchor as? ARFaceAnchor else {
                return nil
            }

            guard let geometry = ARSCNFaceGeometry(device: device) else {
                return nil
            }

            let node = SCNNode(geometry: geometry)
            // Translucent celestial teal 3D facial wireframe mesh
            node.geometry?.firstMaterial?.fillMode = .lines
            node.geometry?.firstMaterial?.diffuse.contents = LiquidGlass.celestialTealMesh
            node.geometry?.firstMaterial?.lightingModel = .constant
            node.geometry?.firstMaterial?.isDoubleSided = true

            self.faceGeometry = geometry
            geometry.update(from: faceAnchor.geometry)
            return node
        }

        public func renderer(_ renderer: SCNSceneRenderer, didUpdate node: SCNNode, for anchor: ARAnchor) {
            guard let faceAnchor = anchor as? ARFaceAnchor, let faceGeometry = self.faceGeometry else {
                return
            }
            faceGeometry.update(from: faceAnchor.geometry)
        }
    }
}
#endif

/// Unified Face Camera view with live TrueDepth camera feed on physical hardware
/// and animated 3D landmark mesh simulation on Simulator.
public struct ARFaceCameraView: View {
    public let session: Any? // ARSession on iOS
    public let isSimulation: Bool
    public let headYawDeg: Double
    public let headPitchDeg: Double
    public let smileIntensity: Double
    public let surpriseScore: Double
    public let distressScore: Double
    public let dominantEmotion: DominantEmotion
    public let isValid: Bool

    public init(
        session: Any? = nil,
        isSimulation: Bool = false,
        headYawDeg: Double = 0.0,
        headPitchDeg: Double = 0.0,
        smileIntensity: Double = 0.0,
        surpriseScore: Double = 0.0,
        distressScore: Double = 0.0,
        dominantEmotion: DominantEmotion = .calm,
        isValid: Bool = true
    ) {
        self.session = session
        self.isSimulation = isSimulation
        self.headYawDeg = headYawDeg
        self.headPitchDeg = headPitchDeg
        self.smileIntensity = smileIntensity
        self.surpriseScore = surpriseScore
        self.distressScore = distressScore
        self.dominantEmotion = dominantEmotion
        self.isValid = isValid
    }

    public var body: some View {
        ZStack {
            #if canImport(ARKit) && canImport(SceneKit) && !os(macOS)
            if let arSession = session as? ARSession, !isSimulation, ARFaceTrackingConfiguration.isSupported {
                ARFaceSceneView(session: arSession)
            } else {
                syntheticFaceMeshView
            }
            #else
            syntheticFaceMeshView
            #endif

            // Outer Face Alignment guide reticle
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(
                    isValid ? LiquidGlass.emeraldMint.opacity(0.8) : LiquidGlass.warmAmber.opacity(0.8),
                    lineWidth: 2
                )
                .padding(4)
        }
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
    }

    // MARK: - Simulator / Fallback Synthetic Face Wireframe

    private var syntheticFaceMeshView: some View {
        GeometryReader { geo in
            let w = geo.size.width
            let h = geo.size.height
            let centerX = w * 0.5 + CGFloat(headYawDeg * 0.7)
            let centerY = h * 0.48 + CGFloat(headPitchDeg * 0.7)

            ZStack {
                // Dark studio gradient background
                LinearGradient(
                    colors: [LiquidGlass.studioMeshTop, LiquidGlass.studioMeshBottom],
                    startPoint: .top,
                    endPoint: .bottom
                )

                // 3D Face Wireframe Contour
                Ellipse()
                    .stroke(LiquidGlass.celestialTeal.opacity(0.4), lineWidth: 1.5)
                    .frame(width: w * 0.62, height: h * 0.68)
                    .position(x: centerX, y: centerY)

                // Eyebrow curves (raises with surprise, furrows with distress)
                let browSurpriseShift: CGFloat = CGFloat(surpriseScore * 10.0)
                let browDistressShift: CGFloat = CGFloat(distressScore * 8.0)
                Path { p in
                    p.move(to: CGPoint(x: centerX - 28, y: centerY - 32 - browSurpriseShift + browDistressShift * 0.5))
                    p.addQuadCurve(
                        to: CGPoint(x: centerX - 8, y: centerY - 34 - browSurpriseShift + browDistressShift),
                        control: CGPoint(x: centerX - 18, y: centerY - 40 - browSurpriseShift)
                    )

                    p.move(to: CGPoint(x: centerX + 8, y: centerY - 34 - browSurpriseShift + browDistressShift))
                    p.addQuadCurve(
                        to: CGPoint(x: centerX + 28, y: centerY - 32 - browSurpriseShift + browDistressShift * 0.5),
                        control: CGPoint(x: centerX + 18, y: centerY - 40 - browSurpriseShift)
                    )
                }
                .stroke(
                    dominantEmotion == .distress ? LiquidGlass.warmAmber : (dominantEmotion == .joy || dominantEmotion == .excited ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal.opacity(0.8)),
                    lineWidth: 2.2
                )

                // Left & Right Eyes with pupil tracking points
                HStack(spacing: 24) {
                    eyePoint
                    eyePoint
                }
                .position(x: centerX, y: centerY - 20)

                // Nose bridge
                Path { p in
                    p.move(to: CGPoint(x: centerX, y: centerY - 16))
                    p.addLine(to: CGPoint(x: centerX - 4, y: centerY + 4))
                    p.addLine(to: CGPoint(x: centerX + 4, y: centerY + 4))
                }
                .stroke(LiquidGlass.celestialTeal.opacity(0.6), lineWidth: 1.5)

                // Dynamic Mouth Contour (curves up for smile, down for frown/distress, opens for surprise)
                Path { p in
                    let smileOffset = CGFloat(smileIntensity * 14.0)
                    let frownOffset = CGFloat(distressScore * 10.0)
                    p.move(to: CGPoint(x: centerX - 18, y: centerY + 24 - smileOffset * 0.2 + frownOffset * 0.2))
                    p.addQuadCurve(
                        to: CGPoint(x: centerX + 18, y: centerY + 24 - smileOffset * 0.2 + frownOffset * 0.2),
                        control: CGPoint(x: centerX, y: centerY + 26 + smileOffset - frownOffset)
                    )
                }
                .stroke(
                    dominantEmotion == .distress ? LiquidGlass.warmAmber : (smileIntensity > 0.35 ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal),
                    style: StrokeStyle(lineWidth: 2.5, lineCap: .round)
                )

                // Tracking mesh dots (cheekbones, chin, temples)
                ForEach(Array(syntheticLandmarks(centerX: centerX, centerY: centerY).enumerated()), id: \.offset) { _, pt in
                    Circle()
                        .fill(LiquidGlass.celestialTeal)
                        .frame(width: 3.5, height: 3.5)
                        .position(pt)
                }
            }
        }
    }

    private var eyePoint: some View {
        ZStack {
            Ellipse()
                .stroke(LiquidGlass.celestialTeal.opacity(0.8), lineWidth: 1.5)
                .frame(width: 18, height: 10)
            Circle()
                .fill(LiquidGlass.emeraldMint)
                .frame(width: 4.5, height: 4.5)
        }
    }

    private func syntheticLandmarks(centerX: CGFloat, centerY: CGFloat) -> [CGPoint] {
        [
            CGPoint(x: centerX - 24, y: centerY + 4),  // Left cheek
            CGPoint(x: centerX + 24, y: centerY + 4),  // Right cheek
            CGPoint(x: centerX, y: centerY + 38),      // Chin
            CGPoint(x: centerX - 30, y: centerY - 14), // Left temple
            CGPoint(x: centerX + 30, y: centerY - 14), // Right temple
            CGPoint(x: centerX, y: centerY - 38)       // Forehead center
        ]
    }
}
