import SwiftUI
import SceneKit
import CurvPalmOverlay

/**
 * Interactive 3D surface mesh and elevation heatmap inspector.
 *
 * Renders the hand topography in continuous 3D coordinate space with:
 * - Real-time orbit camera control and smooth lighting.
 * - Topographic elevation false-color shader (cool blue -> emerald -> gold -> crimson).
 * - Skeletal joint caliper overlay and anatomical pad elevation probes.
 * - Wireframe and surface shading toggles.
 */
public struct HandMeshInspectorView: View {
    public let landmarks: [CGPoint]
    public let padDepths: [String: Float]
    public let referencePalmWidthMm: Float

    @State private var showWireframe: Bool = false
    @State private var showCalipers: Bool = true
    @State private var elevationScale: Float = 1.5
    @State private var scene: SCNScene = SCNScene()
    @State private var sceneId: UUID = UUID()

    public init(
        landmarks: [CGPoint],
        padDepths: [String: Float] = [:],
        referencePalmWidthMm: Float = 80.0
    ) {
        self.landmarks = landmarks
        self.padDepths = padDepths
        self.referencePalmWidthMm = referencePalmWidthMm
    }

    public var body: some View {
        ZStack(alignment: .top) {
            Color.black.ignoresSafeArea()

            if landmarks.count >= 21 {
                SceneView(
                    scene: makeScene(),
                    options: [.allowsCameraControl, .autoenablesDefaultLighting]
                )
                .id(sceneId)
                .ignoresSafeArea()

                // Overlay Controls & Caliper Legend
                VStack(spacing: 12) {
                    topControlBar
                    Spacer()
                    if showCalipers {
                        caliperLegendCard
                    }
                }
                .padding(16)
            } else {
                VStack(spacing: 12) {
                    Image(systemName: "hand.raised.slash")
                        .font(.system(size: 44))
                        .foregroundStyle(.secondary)
                    Text("Insufficient hand tracking points for 3D reconstruction.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }
                .padding(32)
                .frame(maxHeight: .infinity)
            }
        }
    }

    // MARK: - Control Bar

    private var topControlBar: some View {
        HStack(spacing: 12) {
            Toggle(isOn: $showWireframe) {
                Label("Wireframe", systemImage: "square.split.diagonal.2x2")
                    .font(.caption)
            }
            .toggleStyle(.button)
            .tint(Color(hex: "007AFF"))
            .onChange(of: showWireframe) { _ in sceneId = UUID() }

            Toggle(isOn: $showCalipers) {
                Label("Calipers", systemImage: "ruler")
                    .font(.caption)
            }
            .toggleStyle(.button)
            .tint(Color(hex: "34C759"))
            .onChange(of: showCalipers) { _ in sceneId = UUID() }

            Spacer()

            Button {
                sceneId = UUID()
            } label: {
                Image(systemName: "arrow.counterclockwise")
                    .font(.caption)
                    .padding(8)
                    .background(Color.white.opacity(0.15))
                    .clipShape(Circle())
            }
            .foregroundStyle(.white)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    // MARK: - Caliper Legend Card

    private var caliperLegendCard: some View {
        let biometrics = HandBiometricsCalculator.compute(
            landmarks: landmarks.map { [Float($0.x), Float($0.y)] },
            referencePalmWidthMm: referencePalmWidthMm,
            padDepthMm: padDepths
        )

        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("3D Topography & Metrics")
                    .font(.caption.bold())
                    .foregroundStyle(.white)
                Spacer()
                if let ratio = biometrics?.ratios.ratio2D4D {
                    Text(String(format: "2D:4D: %.3f", ratio))
                        .font(.caption.monospacedDigit().bold())
                        .foregroundStyle(Color(hex: "34C759"))
                }
            }

            // Elevation Heatmap gradient legend
            HStack(spacing: 4) {
                Text("-3mm")
                    .font(.system(size: 9, design: .monospaced))
                    .foregroundStyle(.secondary)
                LinearGradient(
                    colors: [
                        Color(red: 0.1, green: 0.4, blue: 0.9),
                        Color(red: 0.1, green: 0.8, blue: 0.4),
                        Color(red: 0.9, green: 0.8, blue: 0.2),
                        Color(red: 0.9, green: 0.2, blue: 0.2)
                    ],
                    startPoint: .leading,
                    endPoint: .trailing
                )
                .frame(height: 6)
                .clipShape(Capsule())
                Text("+6mm")
                    .font(.system(size: 9, design: .monospaced))
                    .foregroundStyle(.secondary)
            }

            if let p = biometrics?.palm {
                HStack {
                    Text(String(format: "Metacarpal Breadth: %.1f mm", p.metacarpalBreadthMm))
                    Spacer()
                    Text(String(format: "Length: %.1f mm", p.palmLengthMm))
                }
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(.secondary)
            }
        }
        .padding(12)
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 14))
    }

    // MARK: - Scene Construction

    private func makeScene() -> SCNScene {
        let scene = SCNScene()
        scene.background.contents = UIColor.black

        guard landmarks.count >= 21 else { return scene }

        // Root Hand Node
        let handNode = SCNNode()

        // 1. Generate 3D Vertices from normalized 2D landmarks + depth
        let (vertices, colors, indices) = buildMeshGeometry()

        let vertexSource = SCNGeometrySource(
            vertices: vertices.map { SCNVector3($0.x, $0.y, $0.z) }
        )
        let normalSource = SCNGeometrySource(
            normals: vertices.map { _ in SCNVector3(0, 0, 1) }
        )
        let colorData = Data(bytes: colors, count: colors.count * MemoryLayout<Float>.size)
        let colorSource = SCNGeometrySource(
            data: colorData,
            semantic: .color,
            vectorCount: colors.count / 4,
            usesFloatComponents: true,
            componentsPerVector: 4,
            bytesPerComponent: MemoryLayout<Float>.size,
            dataOffset: 0,
            dataStride: MemoryLayout<Float>.size * 4
        )

        let element = SCNGeometryElement(
            indices: indices,
            primitiveType: .triangles
        )

        let geometry = SCNGeometry(sources: [vertexSource, normalSource, colorSource], elements: [element])
        let material = SCNMaterial()
        material.isDoubleSided = true
        material.fillMode = showWireframe ? .lines : .fill
        material.diffuse.contents = UIColor.white
        geometry.materials = [material]

        let meshNode = SCNNode(geometry: geometry)
        handNode.addChildNode(meshNode)

        // 2. Skeletal joint nodes & connections
        if showCalipers {
            for (idx, v) in vertices.prefix(21).enumerated() {
                let sphere = SCNSphere(radius: idx % 4 == 0 ? 0.0035 : 0.0022)
                sphere.firstMaterial?.diffuse.contents = (idx == 8 || idx == 16)
                    ? UIColor(red: 0.2, green: 0.9, blue: 0.5, alpha: 1.0)
                    : UIColor(white: 0.85, alpha: 0.9)
                let sphereNode = SCNNode(geometry: sphere)
                sphereNode.position = SCNVector3(v.x, v.y, v.z)
                handNode.addChildNode(sphereNode)
            }
        }

        scene.rootNode.addChildNode(handNode)

        // Camera
        let cameraNode = SCNNode()
        cameraNode.camera = SCNCamera()
        cameraNode.position = SCNVector3(0, 0, 0.28)
        scene.rootNode.addChildNode(cameraNode)

        // Ambient & Directional Lighting
        let ambient = SCNLight()
        ambient.type = .ambient
        ambient.color = UIColor(white: 0.45, alpha: 1.0)
        let ambientNode = SCNNode()
        ambientNode.light = ambient
        scene.rootNode.addChildNode(ambientNode)

        let light = SCNLight()
        light.type = .directional
        light.color = UIColor(white: 0.8, alpha: 1.0)
        let lightNode = SCNNode()
        lightNode.light = light
        lightNode.position = SCNVector3(0.1, 0.2, 0.3)
        scene.rootNode.addChildNode(lightNode)

        return scene
    }

    // MARK: - Mesh Geometry Builder

    private struct Vertex3D {
        var x: Float
        var y: Float
        var z: Float
    }

    private func buildMeshGeometry() -> (vertices: [Vertex3D], colors: [Float], indices: [Int32]) {
        var vertices: [Vertex3D] = []
        var colors: [Float] = [] // RGBA floats

        // Scale normalized [0..1] coordinates to metric meters (~0.12m span)
        let scale: Float = 0.16

        // Compute centroid
        var cx: Float = 0
        var cy: Float = 0
        for pt in landmarks.prefix(21) {
            cx += Float(pt.x)
            cy += Float(pt.y)
        }
        cx /= 21.0
        cy /= 21.0

        for (idx, pt) in landmarks.prefix(21).enumerated() {
            let x = (Float(pt.x) - cx) * scale
            let y = -(Float(pt.y) - cy) * scale // flip upright

            // Z elevation: compute based on pad depths
            var zMm: Float = 0.0
            if idx == 1 || idx == 2 {
                zMm = padDepths["thenar"] ?? padDepths["venus"] ?? 1.5
            } else if idx == 5 || idx == 6 {
                zMm = padDepths["radial_interdigital"] ?? padDepths["jupiter"] ?? 0.8
            } else if idx == 9 || idx == 10 {
                zMm = padDepths["central_interdigital"] ?? padDepths["saturn"] ?? 0.6
            } else if idx == 13 || idx == 14 {
                zMm = padDepths["ring_interdigital"] ?? padDepths["apollo"] ?? 0.7
            } else if idx == 17 || idx == 18 {
                zMm = padDepths["ulnar_interdigital"] ?? padDepths["mercury"] ?? 0.5
            } else if idx == 0 {
                zMm = -0.5
            }

            let z = (zMm * 0.001) * elevationScale
            vertices.append(Vertex3D(x: x, y: y, z: z))

            // Colormap by elevation: [-2mm .. +4mm] -> [0 .. 1]
            let t = max(0.0, min(1.0, (zMm + 2.0) / 6.0))
            let (r, g, b) = elevationColor(t: t)
            colors.append(contentsOf: [r, g, b, 1.0])
        }

        // Add palm center vertex for fan triangulation
        let palmCenterIdx = Int32(vertices.count)
        let palmZ = ((padDepths["palmar_fossa"] ?? padDepths["mars"] ?? -1.0) * 0.001) * elevationScale
        vertices.append(Vertex3D(x: 0, y: -0.015, z: palmZ))
        let (fR, fG, fB) = elevationColor(t: 0.15)
        colors.append(contentsOf: [fR, fG, fB, 1.0])

        // Anatomical Triangulation Index Buffer
        var indices: [Int32] = []

        // Palm fan connected to wrist & MCPs
        let palmRing: [Int32] = [0, 1, 2, 5, 9, 13, 17, 0]
        for i in 0..<(palmRing.count - 1) {
            indices.append(contentsOf: [palmCenterIdx, palmRing[i], palmRing[i + 1]])
        }

        // Interdigital webs & MCP bridges
        indices.append(contentsOf: [2, 5, palmCenterIdx])
        indices.append(contentsOf: [5, 6, 9])
        indices.append(contentsOf: [6, 10, 9])
        indices.append(contentsOf: [9, 10, 13])
        indices.append(contentsOf: [10, 14, 13])
        indices.append(contentsOf: [13, 14, 17])
        indices.append(contentsOf: [14, 18, 17])

        // Intermediate interdigital webbing
        indices.append(contentsOf: [6, 7, 10])
        indices.append(contentsOf: [7, 11, 10])
        indices.append(contentsOf: [10, 11, 14])
        indices.append(contentsOf: [11, 15, 14])
        indices.append(contentsOf: [14, 15, 18])
        indices.append(contentsOf: [15, 19, 18])

        return (vertices, colors, indices)
    }

    private func elevationColor(t: Float) -> (Float, Float, Float) {
        if t < 0.33 {
            let u = t / 0.33
            return (0.1 + 0.1 * u, 0.4 + 0.4 * u, 0.9 - 0.4 * u) // blue -> cyan
        } else if t < 0.66 {
            let u = (t - 0.33) / 0.33
            return (0.2 + 0.7 * u, 0.8 + 0.1 * u, 0.5 - 0.3 * u) // cyan -> emerald/yellow
        } else {
            let u = (t - 0.66) / 0.34
            return (0.9, 0.9 - 0.7 * u, 0.2 - 0.1 * u) // yellow -> crimson
        }
    }
}
