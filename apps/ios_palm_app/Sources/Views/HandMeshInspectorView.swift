import SwiftUI
import SceneKit
import simd
import CurvPalmOverlay

/**
 * Interactive 3D surface mesh and elevation heatmap inspector.
 *
 * Renders the hand topography in continuous 3D coordinate space with:
 * - Real-time orbit camera control and dual-directional + ambient lighting.
 * - Non-clipping camera frustum (zNear = 0.001m, auto Z range).
 * - Full anatomical surface mesh covering palm, thenar/hypothenar mounts, and all 5 digits.
 * - Topographic elevation colormap (cool blue -> cyan -> emerald -> gold -> crimson).
 * - Skeletal joint calipers with 3D kinematic bones and 2D:4D ratio measurement span.
 * - 3D billboard pins for the 7 planetary/anatomical mounts with metric elevations.
 * - Wireframe and elevation relief exaggeration controls.
 */
public struct HandMeshInspectorView: View {
    public let landmarks: [CGPoint]
    public let padDepths: [String: Float]
    public let referencePalmWidthMm: Float

    @State private var showWireframe: Bool = false
    @State private var showCalipers: Bool = true
    @State private var showMountPins: Bool = true
    @State private var elevationScale: Float = 1.2
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
                    caliperLegendCard
                }
                .padding(16)
            } else {
                insufficientLandmarksView
            }
        }
    }

    // MARK: - Insufficient Data View

    private var insufficientLandmarksView: some View {
        VStack(spacing: 16) {
            Image(systemName: "hand.raised.slash")
                .font(.system(size: 48))
                .foregroundStyle(.secondary)
            Text("Insufficient Hand Tracking Points")
                .font(.headline)
                .foregroundStyle(Color.primary)
            Text("3D surface reconstruction requires a clear view of all 21 hand landmarks.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(32)
        .frame(maxHeight: .infinity)
    }

    // MARK: - Control Bar

    private var topControlBar: some View {
        HStack(spacing: 8) {
            Toggle(isOn: $showWireframe) {
                Label("Wireframe", systemImage: "square.split.diagonal.2x2")
                    .font(.caption2.bold())
            }
            .toggleStyle(.button)
            .tint(Color(hex: "007AFF"))
            .onChange(of: showWireframe) { _ in sceneId = UUID() }

            Toggle(isOn: $showCalipers) {
                Label("Calipers", systemImage: "ruler")
                    .font(.caption2.bold())
            }
            .toggleStyle(.button)
            .tint(Color(hex: "34C759"))
            .onChange(of: showCalipers) { _ in sceneId = UUID() }

            Toggle(isOn: $showMountPins) {
                Label("Mounts", systemImage: "mappin.and.ellipse")
                    .font(.caption2.bold())
            }
            .toggleStyle(.button)
            .tint(Color(hex: "FF9500"))
            .onChange(of: showMountPins) { _ in sceneId = UUID() }

            // Relief exaggeration cycler
            Button {
                if elevationScale < 1.3 {
                    elevationScale = 1.4
                } else if elevationScale < 1.7 {
                    elevationScale = 1.8
                } else {
                    elevationScale = 1.0
                }
                sceneId = UUID()
            } label: {
                Text(String(format: "%.1fx 3D", elevationScale))
                    .font(.caption2.monospacedDigit().bold())
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .background(Color.secondary.opacity(0.18))
                    .clipShape(Capsule())
            }
            .foregroundStyle(Color.primary)

            Spacer()

            Button {
                sceneId = UUID()
            } label: {
                Image(systemName: "arrow.counterclockwise")
                    .font(.caption)
                    .padding(8)
                    .background(Color.secondary.opacity(0.18))
                    .clipShape(Circle())
            }
            .foregroundStyle(Color.primary)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 14))
    }

    // MARK: - Caliper Legend Card

    private var caliperLegendCard: some View {
        let biometrics = HandBiometricsCalculator.compute(
            landmarks: landmarks.map { [Float($0.x), Float($0.y)] },
            referencePalmWidthMm: referencePalmWidthMm,
            padDepthMm: padDepths
        )

        return VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("3D Topography & Biomarkers")
                    .font(.caption.bold())
                    .foregroundStyle(Color.primary)
                Spacer()
                if let ratio = biometrics?.ratios.ratio2D4D {
                    HStack(spacing: 4) {
                        Circle()
                            .fill(Color(hex: "34C759"))
                            .frame(width: 6, height: 6)
                        Text(String(format: "2D:4D: %.3f", ratio))
                            .font(.caption.monospacedDigit().bold())
                            .foregroundStyle(Color(hex: "34C759"))
                    }
                }
            }

            // Elevation Heatmap gradient legend
            HStack(spacing: 6) {
                Text("-3mm")
                    .font(.system(size: 9, design: .monospaced))
                    .foregroundStyle(.secondary)
                LinearGradient(
                    colors: [
                        Color(red: 0.1, green: 0.4, blue: 0.9),
                        Color(red: 0.1, green: 0.8, blue: 0.7),
                        Color(red: 0.2, green: 0.85, blue: 0.4),
                        Color(red: 0.95, green: 0.8, blue: 0.2),
                        Color(red: 0.95, green: 0.25, blue: 0.2)
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
                    Text(String(format: "Breadth: %.1f mm", p.metacarpalBreadthMm))
                    Spacer()
                    Text(String(format: "Length: %.1f mm", p.palmLengthMm))
                    Spacer()
                    Text(p.handShapeType.capitalized)
                        .foregroundStyle(Color(hex: "34C759"))
                }
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(.secondary)
            }

            // Prominent Mount Heights Chips
            if !padDepths.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        mountChip(name: "Venus", val: padDepths["venus"] ?? padDepths["thenar"])
                        mountChip(name: "Jupiter", val: padDepths["jupiter"] ?? padDepths["radial_interdigital"])
                        mountChip(name: "Saturn", val: padDepths["saturn"] ?? padDepths["central_interdigital"])
                        mountChip(name: "Apollo", val: padDepths["apollo"] ?? padDepths["ring_interdigital"])
                        mountChip(name: "Mercury", val: padDepths["mercury"] ?? padDepths["ulnar_interdigital"])
                        mountChip(name: "Luna", val: padDepths["luna"] ?? padDepths["hypothenar"])
                        mountChip(name: "Mars", val: padDepths["mars"] ?? padDepths["palmar_fossa"])
                    }
                }
            }
        }
        .padding(12)
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 14))
    }

    @ViewBuilder
    private func mountChip(name: String, val: Float?) -> some View {
        if let val {
            HStack(spacing: 3) {
                Text(name)
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(.secondary)
                Text(String(format: "%+.1fmm", val))
                    .font(.system(size: 9, weight: .bold, design: .monospaced))
                    .foregroundStyle(val >= 1.5 ? Color(hex: "34C759") : (val < 0 ? Color(hex: "007AFF") : Color.primary))
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(Color.secondary.opacity(0.12))
            .clipShape(Capsule())
        }
    }

    // MARK: - Scene Construction

    private func makeScene() -> SCNScene {
        let scene = SCNScene()
        scene.background.contents = UIColor.black

        guard landmarks.count >= 21 else { return scene }

        let handNode = SCNNode()

        // 1. Generate 3D Vertices from normalized 2D landmarks + depth
        let meshResult = HandMeshGeometryBuilder.buildMeshGeometry(
            landmarks: landmarks,
            padDepths: padDepths,
            referencePalmWidthMm: referencePalmWidthMm,
            elevationScale: elevationScale
        )

        let vertexSource = SCNGeometrySource(
            vertices: meshResult.vertices.map { SCNVector3($0.x, $0.y, $0.z) }
        )
        let normalSource = SCNGeometrySource(
            normals: HandMeshGeometryBuilder.computeNormals(vertices: meshResult.vertices, indices: meshResult.indices)
        )
        let colorData = Data(bytes: meshResult.colors, count: meshResult.colors.count * MemoryLayout<Float>.size)
        let colorSource = SCNGeometrySource(
            data: colorData,
            semantic: .color,
            vectorCount: meshResult.colors.count / 4,
            usesFloatComponents: true,
            componentsPerVector: 4,
            bytesPerComponent: MemoryLayout<Float>.size,
            dataOffset: 0,
            dataStride: MemoryLayout<Float>.size * 4
        )

        let element = SCNGeometryElement(
            indices: meshResult.indices,
            primitiveType: .triangles
        )

        let geometry = SCNGeometry(sources: [vertexSource, normalSource, colorSource], elements: [element])
        let material = SCNMaterial()
        material.isDoubleSided = true
        // Phong shading gives the surface a natural depth and highlights that read as 3D skin
        material.lightingModel = showWireframe ? .constant : .phong
        material.fillMode = showWireframe ? .lines : .fill
        material.diffuse.contents = UIColor.white
        // Soft specular gives a subtle skin-like sheen over the elevation heatmap
        material.specular.contents = UIColor(white: 0.28, alpha: 1.0)
        material.shininess = 18
        geometry.materials = [material]

        let meshNode = SCNNode(geometry: geometry)
        handNode.addChildNode(meshNode)

        // 2. Scan-point landmark dots: emissive glow on each of the 21 tracked points
        //    These make it unmistakably feel like a structured-light scanner result.
        let scanPointNode = makeScanPointCloud(landmarks: meshResult.landmark3DPositions)
        handNode.addChildNode(scanPointNode)

        // 3. Skeletal joint nodes & connecting caliper bones
        if showCalipers && meshResult.landmark3DPositions.count >= 21 {
            let caliperNode = makeCaliperHierarchy(landmarks: meshResult.landmark3DPositions)
            handNode.addChildNode(caliperNode)
        }

        // 4. 3D Mount Elevation Pins
        if showMountPins && meshResult.landmark3DPositions.count >= 21 {
            let pinsNode = makeMountPinsHierarchy(landmarks: meshResult.landmark3DPositions)
            handNode.addChildNode(pinsNode)
        }

        scene.rootNode.addChildNode(handNode)

        // Camera: closer FOV (42°) fills the screen with the hand, feels like a real scan viewer
        let cameraNode = SCNNode()
        cameraNode.name = "primaryCamera"
        let camera = SCNCamera()
        camera.zNear = 0.001
        camera.zFar = 5.0
        camera.automaticallyAdjustsZRange = true
        camera.fieldOfView = 42.0  // tighter: hand fills more of the frame
        cameraNode.camera = camera
        cameraNode.position = SCNVector3(0, 0.015, 0.22)
        cameraNode.look(at: SCNVector3(0, 0.015, 0))
        scene.rootNode.addChildNode(cameraNode)

        // Ambient: keep dark so specular lights pop against the elevation heatmap
        let ambient = SCNLight()
        ambient.type = .ambient
        ambient.color = UIColor(white: 0.45, alpha: 1.0)
        let ambientNode = SCNNode()
        ambientNode.light = ambient
        scene.rootNode.addChildNode(ambientNode)

        // Key light: upper-right front — main shape definition
        let keyLight = SCNLight()
        keyLight.type = .directional
        keyLight.color = UIColor(white: 0.90, alpha: 1.0)
        let keyLightNode = SCNNode()
        keyLightNode.light = keyLight
        keyLightNode.position = SCNVector3(0.12, 0.22, 0.30)
        keyLightNode.look(at: SCNVector3(0, 0, 0))
        scene.rootNode.addChildNode(keyLightNode)

        // Fill light: lower-left front — softens shadows on the palm side
        let fillLight = SCNLight()
        fillLight.type = .directional
        fillLight.color = UIColor(white: 0.40, alpha: 1.0)
        let fillLightNode = SCNNode()
        fillLightNode.light = fillLight
        fillLightNode.position = SCNVector3(-0.12, -0.10, 0.22)
        fillLightNode.look(at: SCNVector3(0, 0, 0))
        scene.rootNode.addChildNode(fillLightNode)

        // Rim light: behind + right — creates edge separation from background, depth illusion
        let rimLight = SCNLight()
        rimLight.type = .directional
        rimLight.color = UIColor(red: 0.2, green: 0.85, blue: 0.75, alpha: 0.55)
        let rimLightNode = SCNNode()
        rimLightNode.light = rimLight
        rimLightNode.position = SCNVector3(0.20, 0.10, -0.30)
        rimLightNode.look(at: SCNVector3(0, 0, 0))
        scene.rootNode.addChildNode(rimLightNode)

        return scene
    }

    // MARK: - Scan-Point Cloud Overlay

    /// Creates emissive glow spheres on all 21 MediaPipe landmarks.
    /// Wrist = amber anchor, MCPs = cyan, fingertips = bright green, PIP/DIP = small white.
    private func makeScanPointCloud(landmarks: [SCNVector3]) -> SCNNode {
        let root = SCNNode()
        guard landmarks.count >= 21 else { return root }

        let mcpSet: Set<Int> = [1, 5, 9, 13, 17]
        let tipSet: Set<Int> = [4, 8, 12, 16, 20]

        for (idx, pos) in landmarks.prefix(21).enumerated() {
            let (dotRadius, dotColor): (CGFloat, UIColor)
            if idx == 0 {
                dotRadius = 0.0026
                dotColor = UIColor(red: 1.0, green: 0.60, blue: 0.0, alpha: 1.0)
            } else if mcpSet.contains(idx) {
                dotRadius = 0.0020
                dotColor = UIColor(red: 0.15, green: 0.88, blue: 0.82, alpha: 1.0)
            } else if tipSet.contains(idx) {
                dotRadius = 0.0018
                dotColor = UIColor(red: 0.30, green: 0.98, blue: 0.55, alpha: 1.0)
            } else {
                dotRadius = 0.0012
                dotColor = UIColor(white: 0.95, alpha: 0.85)
            }

            let sphere = SCNSphere(radius: dotRadius)
            sphere.segmentCount = 8  // low-poly spheres keep scene fast
            let mat = SCNMaterial()
            mat.diffuse.contents = dotColor
            // Strong emission makes them glow like structured-light scan points
            mat.emission.contents = dotColor.withAlphaComponent(0.80)
            mat.lightingModel = .constant
            sphere.materials = [mat]

            let node = SCNNode(geometry: sphere)
            node.position = SCNVector3(pos.x, pos.y, pos.z + 0.0004) // float just above mesh
            root.addChildNode(node)
        }
        return root
    }

    // MARK: - Caliper & Bone Hierarchy

    private func makeCaliperHierarchy(landmarks: [SCNVector3]) -> SCNNode {
        let root = SCNNode()

        // Kinematic bone chains
        let chains: [[Int]] = [
            [0, 1, 2, 3, 4],
            [0, 5, 6, 7, 8],
            [0, 9, 10, 11, 12],
            [0, 13, 14, 15, 16],
            [0, 17, 18, 19, 20],
            [1, 2, 5, 9, 13, 17] // Metacarpal transverse arch
        ]

        for chain in chains {
            for j in 0..<(chain.count - 1) {
                let idxA = chain[j]
                let idxB = chain[j + 1]
                let pA = landmarks[idxA]
                let pB = landmarks[idxB]
                let isArch = chain.count == 6
                let color = isArch
                    ? UIColor(red: 0.0, green: 0.78, blue: 0.75, alpha: 0.7)
                    : UIColor(white: 0.92, alpha: 0.85)
                let radius: CGFloat = isArch ? 0.0008 : 0.0011
                let bone = makeCylinderNode(from: pA, to: pB, radius: radius, color: color)
                root.addChildNode(bone)
            }
        }

        // 2D:4D Measurement span beam between index tip (8) and ring tip (16)
        let p8 = landmarks[8]
        let p16 = landmarks[16]
        let caliperBeam = makeCylinderNode(
            from: p8,
            to: p16,
            radius: 0.0009,
            color: UIColor(red: 0.2, green: 0.85, blue: 0.4, alpha: 0.85)
        )
        root.addChildNode(caliperBeam)

        // Joint Spheres
        for (idx, p) in landmarks.prefix(21).enumerated() {
            var radius: CGFloat = 0.0022
            var color = UIColor(white: 0.85, alpha: 0.9)

            if idx == 0 {
                // Wrist
                radius = 0.0036
                color = UIColor(red: 1.0, green: 0.58, blue: 0.0, alpha: 1.0)
            } else if idx == 1 || idx == 5 || idx == 9 || idx == 13 || idx == 17 {
                // MCP Knuckles
                radius = 0.0030
                color = UIColor(red: 0.0, green: 0.78, blue: 0.75, alpha: 1.0)
            } else if idx == 8 || idx == 16 {
                // Key 2D:4D Biomarker Digits (Index & Ring tips)
                radius = 0.0038
                color = UIColor(red: 0.2, green: 0.9, blue: 0.45, alpha: 1.0)
            }

            let sphere = SCNSphere(radius: radius)
            let sphereMat = SCNMaterial()
            sphereMat.diffuse.contents = color
            sphereMat.emission.contents = color.withAlphaComponent(0.3)
            sphere.materials = [sphereMat]

            let sphereNode = SCNNode(geometry: sphere)
            sphereNode.position = p
            root.addChildNode(sphereNode)
        }

        return root
    }

    // MARK: - Mount Elevation 3D Pins

    private func makeMountPinsHierarchy(landmarks: [SCNVector3]) -> SCNNode {
        let root = SCNNode()

        let mountProbes: [(name: String, anchorIdx: Int, depthKey: String, offset: SCNVector3)] = [
            ("Venus", 1, "venus", SCNVector3(-0.008, -0.005, 0.003)),
            ("Jupiter", 5, "jupiter", SCNVector3(-0.004, -0.006, 0.003)),
            ("Saturn", 9, "saturn", SCNVector3(0, -0.006, 0.003)),
            ("Apollo", 13, "apollo", SCNVector3(0.004, -0.006, 0.003)),
            ("Mercury", 17, "mercury", SCNVector3(0.008, -0.006, 0.003)),
            ("Luna", 17, "luna", SCNVector3(0.012, -0.028, 0.002))
        ]

        for probe in mountProbes {
            guard probe.anchorIdx < landmarks.count else { continue }
            let base = landmarks[probe.anchorIdx]
            let pos = SCNVector3(
                base.x + probe.offset.x,
                base.y + probe.offset.y,
                base.z + probe.offset.z
            )
            let hMm = padDepths[probe.depthKey] ?? padDepths[probe.name.lowercased()] ?? 1.5

            let pin = makePinNode(at: pos, title: probe.name, valMm: hMm)
            root.addChildNode(pin)
        }

        return root
    }

    private func makePinNode(at position: SCNVector3, title: String, valMm: Float) -> SCNNode {
        let pinNode = SCNNode()
        pinNode.position = position

        // Small indicator dot
        let dot = SCNSphere(radius: 0.0022)
        let dotMat = SCNMaterial()
        let dotColor = valMm >= 1.5
            ? UIColor(red: 0.2, green: 0.85, blue: 0.4, alpha: 1.0)
            : UIColor(red: 0.95, green: 0.65, blue: 0.2, alpha: 1.0)
        dotMat.diffuse.contents = dotColor
        dotMat.emission.contents = dotColor.withAlphaComponent(0.4)
        dot.materials = [dotMat]
        let dotNode = SCNNode(geometry: dot)
        pinNode.addChildNode(dotNode)

        return pinNode
    }

    // MARK: - Cylinder / Bone Helper

    private func makeCylinderNode(
        from p1: SCNVector3,
        to p2: SCNVector3,
        radius: CGFloat,
        color: UIColor
    ) -> SCNNode {
        let dx = p2.x - p1.x
        let dy = p2.y - p1.y
        let dz = p2.z - p1.z
        let distance = sqrt(dx * dx + dy * dy + dz * dz)
        guard distance > 0.0001 else { return SCNNode() }

        let cylinder = SCNCylinder(radius: radius, height: CGFloat(distance))
        cylinder.radialSegmentCount = 8
        let mat = SCNMaterial()
        mat.diffuse.contents = color
        mat.lightingModel = .constant
        cylinder.materials = [mat]

        let node = SCNNode(geometry: cylinder)
        node.position = SCNVector3((p1.x + p2.x) / 2, (p1.y + p2.y) / 2, (p1.z + p2.z) / 2)

        let dir = simd_normalize(simd_float3(dx, dy, dz))
        let yAxis = simd_float3(0, 1, 0)
        let dotProduct = simd_dot(yAxis, dir)

        if abs(dotProduct - 1.0) < 0.0001 {
            // Already aligned with +Y
        } else if abs(dotProduct + 1.0) < 0.0001 {
            node.simdOrientation = simd_quatf(angle: .pi, axis: simd_float3(1, 0, 0))
        } else {
            let axis = simd_normalize(simd_cross(yAxis, dir))
            let angle = acos(max(-1.0, min(1.0, dotProduct)))
            node.simdOrientation = simd_quatf(angle: angle, axis: axis)
        }

        return node
    }
}
