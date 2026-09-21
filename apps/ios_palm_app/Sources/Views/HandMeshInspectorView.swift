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
    @State private var elevationScale: Float = 1.8
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
                if elevationScale < 1.4 {
                    elevationScale = 1.8
                } else if elevationScale < 2.2 {
                    elevationScale = 2.8
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
        let (vertices, colors, indices, landmark3DPositions) = buildMeshGeometry()

        let vertexSource = SCNGeometrySource(
            vertices: vertices.map { SCNVector3($0.x, $0.y, $0.z) }
        )
        let normalSource = SCNGeometrySource(
            normals: computeNormals(vertices: vertices, indices: indices)
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
        material.lightingModel = .lambert
        material.fillMode = showWireframe ? .lines : .fill
        material.diffuse.contents = UIColor.white
        geometry.materials = [material]

        let meshNode = SCNNode(geometry: geometry)
        handNode.addChildNode(meshNode)

        // 2. Skeletal joint nodes & connecting caliper bones
        if showCalipers && landmark3DPositions.count >= 21 {
            let caliperNode = makeCaliperHierarchy(landmarks: landmark3DPositions)
            handNode.addChildNode(caliperNode)
        }

        // 3. 3D Mount Elevation Pins
        if showMountPins && landmark3DPositions.count >= 21 {
            let pinsNode = makeMountPinsHierarchy(landmarks: landmark3DPositions)
            handNode.addChildNode(pinsNode)
        }

        scene.rootNode.addChildNode(handNode)

        // Camera: zNear must be tiny (0.001m) to prevent clipping at close distance!
        let cameraNode = SCNNode()
        cameraNode.name = "primaryCamera"
        let camera = SCNCamera()
        camera.zNear = 0.001
        camera.zFar = 10.0
        camera.automaticallyAdjustsZRange = true
        camera.fieldOfView = 50.0
        cameraNode.camera = camera
        cameraNode.position = SCNVector3(0, -0.012, 0.28)
        cameraNode.look(at: SCNVector3(0, -0.012, 0))
        scene.rootNode.addChildNode(cameraNode)

        // Ambient & Directional Lighting
        let ambient = SCNLight()
        ambient.type = .ambient
        ambient.color = UIColor(white: 0.65, alpha: 1.0)
        let ambientNode = SCNNode()
        ambientNode.light = ambient
        scene.rootNode.addChildNode(ambientNode)

        // Key light (upper-right front)
        let keyLight = SCNLight()
        keyLight.type = .directional
        keyLight.color = UIColor(white: 0.85, alpha: 1.0)
        let keyLightNode = SCNNode()
        keyLightNode.light = keyLight
        keyLightNode.position = SCNVector3(0.15, 0.25, 0.35)
        keyLightNode.look(at: SCNVector3(0, 0, 0))
        scene.rootNode.addChildNode(keyLightNode)

        // Fill light (lower-left front)
        let fillLight = SCNLight()
        fillLight.type = .directional
        fillLight.color = UIColor(white: 0.45, alpha: 1.0)
        let fillLightNode = SCNNode()
        fillLightNode.light = fillLight
        fillLightNode.position = SCNVector3(-0.15, -0.15, 0.25)
        fillLightNode.look(at: SCNVector3(0, 0, 0))
        scene.rootNode.addChildNode(fillLightNode)

        return scene
    }

    // MARK: - Mesh Geometry Builder

    private struct Vertex3D {
        var x: Float
        var y: Float
        var z: Float
    }

    private func buildMeshGeometry() -> (
        vertices: [Vertex3D],
        colors: [Float],
        indices: [Int32],
        landmarkPositions: [SCNVector3]
    ) {
        var vertices: [Vertex3D] = []
        var colors: [Float] = []
        var landmark3D: [SCNVector3] = []

        // Human hand span metric scale (~0.18m wrist to middle fingertip)
        let scale: Float = 0.18

        // Compute centroid of the 21 landmarks
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
                zMm = padDepths["thenar"] ?? padDepths["venus"] ?? 2.5
            } else if idx == 5 || idx == 6 {
                zMm = padDepths["radial_interdigital"] ?? padDepths["jupiter"] ?? 1.8
            } else if idx == 9 || idx == 10 {
                zMm = padDepths["central_interdigital"] ?? padDepths["saturn"] ?? 1.5
            } else if idx == 13 || idx == 14 {
                zMm = padDepths["ring_interdigital"] ?? padDepths["apollo"] ?? 1.6
            } else if idx == 17 || idx == 18 {
                zMm = padDepths["ulnar_interdigital"] ?? padDepths["mercury"] ?? 1.2
            } else if idx == 0 {
                zMm = -0.5
            } else {
                zMm = 0.4
            }

            let z = (zMm * 0.001) * elevationScale
            vertices.append(Vertex3D(x: x, y: y, z: z))
            landmark3D.append(SCNVector3(x, y, z))

            let t = max(0.0, min(1.0, (zMm + 2.0) / 6.0))
            let (r, g, b) = elevationColor(t: t)
            colors.append(contentsOf: [r, g, b, 1.0])
        }

        // Add palm center vertex for fan triangulation
        let palmCenterIdx = Int32(vertices.count)
        let palmZ = ((padDepths["palmar_fossa"] ?? padDepths["mars"] ?? -1.5) * 0.001) * elevationScale
        vertices.append(Vertex3D(x: 0, y: -0.015, z: palmZ))
        let (fR, fG, fB) = elevationColor(t: 0.12)
        colors.append(contentsOf: [fR, fG, fB, 1.0])

        // Add Luna (hypothenar) pad vertex
        let lunaIdx = Int32(vertices.count)
        let v0 = vertices[0]
        let v17 = vertices[17]
        let lunaZ = ((padDepths["hypothenar"] ?? padDepths["luna"] ?? 2.0) * 0.001) * elevationScale
        vertices.append(Vertex3D(
            x: (v0.x * 0.4 + v17.x * 0.6) + 0.008,
            y: (v0.y * 0.4 + v17.y * 0.6),
            z: lunaZ
        ))
        let (lR, lG, lB) = elevationColor(t: 0.65)
        colors.append(contentsOf: [lR, lG, lB, 1.0])

        // Add Venus (thenar) pad center vertex
        let venusIdx = Int32(vertices.count)
        let v1 = vertices[1]
        let v2 = vertices[2]
        let venusZ = (((padDepths["thenar"] ?? padDepths["venus"] ?? 2.5) + 0.8) * 0.001) * elevationScale
        vertices.append(Vertex3D(
            x: (v0.x * 0.3 + v1.x * 0.35 + v2.x * 0.35) - 0.006,
            y: (v0.y * 0.3 + v1.y * 0.35 + v2.y * 0.35),
            z: venusZ
        ))
        let (vR, vG, vB) = elevationColor(t: 0.82)
        colors.append(contentsOf: [vR, vG, vB, 1.0])

        // Anatomical Triangulation Index Buffer
        var indices: [Int32] = []

        // Palm fan connected to wrist, thenar, MCPs, and hypothenar
        let palmRing: [Int32] = [0, venusIdx, 1, 2, 5, 9, 13, 17, lunaIdx, 0]
        for i in 0..<(palmRing.count - 1) {
            indices.append(contentsOf: [palmCenterIdx, palmRing[i], palmRing[i + 1]])
        }

        // Interdigital webbing triangles
        indices.append(contentsOf: [2, 5, palmCenterIdx])
        indices.append(contentsOf: [5, 6, 9])
        indices.append(contentsOf: [6, 10, 9])
        indices.append(contentsOf: [9, 10, 13])
        indices.append(contentsOf: [10, 14, 13])
        indices.append(contentsOf: [13, 14, 17])
        indices.append(contentsOf: [14, 18, 17])

        indices.append(contentsOf: [6, 7, 10])
        indices.append(contentsOf: [7, 11, 10])
        indices.append(contentsOf: [10, 11, 14])
        indices.append(contentsOf: [11, 15, 14])
        indices.append(contentsOf: [14, 15, 18])
        indices.append(contentsOf: [15, 19, 18])

        // Complete 5-digit 3D ribbon mesh
        let digits: [[Int]] = [
            [1, 2, 3, 4],       // Thumb
            [5, 6, 7, 8],       // Index
            [9, 10, 11, 12],    // Middle
            [13, 14, 15, 16],   // Ring
            [17, 18, 19, 20]    // Pinky
        ]
        let fingerWidths: [Float] = [0.008, 0.007, 0.007, 0.0065, 0.0055]

        for (dIdx, chain) in digits.enumerated() {
            let baseWidth = fingerWidths[dIdx]
            var leftIndices: [Int32] = []
            var rightIndices: [Int32] = []

            for (segIdx, jointIdx) in chain.enumerated() {
                let jPt = vertices[jointIdx]
                let taper = 1.0 - (Float(segIdx) * 0.18)
                let halfW = baseWidth * taper

                // Compute normal perpendicular to finger segment direction
                let nextIdx = segIdx < chain.count - 1 ? chain[segIdx + 1] : jointIdx
                let prevIdx = segIdx > 0 ? chain[segIdx - 1] : jointIdx
                let refPtA = vertices[prevIdx]
                let refPtB = vertices[nextIdx]
                var dx = refPtB.x - refPtA.x
                var dy = refPtB.y - refPtA.y
                let len = sqrt(dx * dx + dy * dy)
                if len > 0.0001 {
                    dx /= len
                    dy /= len
                } else {
                    dx = 0
                    dy = 1
                }
                let perpX = -dy * halfW
                let perpY = dx * halfW

                let lIdx = Int32(vertices.count)
                vertices.append(Vertex3D(x: jPt.x + perpX, y: jPt.y + perpY, z: jPt.z))
                colors.append(contentsOf: [0.35, 0.75, 0.55, 0.95])
                leftIndices.append(lIdx)

                let rIdx = Int32(vertices.count)
                vertices.append(Vertex3D(x: jPt.x - perpX, y: jPt.y - perpY, z: jPt.z))
                colors.append(contentsOf: [0.35, 0.75, 0.55, 0.95])
                rightIndices.append(rIdx)
            }

            // Triangulate finger quads
            for s in 0..<(chain.count - 1) {
                let lA = leftIndices[s]
                let rA = rightIndices[s]
                let lB = leftIndices[s + 1]
                let rB = rightIndices[s + 1]
                let cA = Int32(chain[s])
                let cB = Int32(chain[s + 1])

                indices.append(contentsOf: [lA, cA, lB])
                indices.append(contentsOf: [cA, cB, lB])
                indices.append(contentsOf: [cA, rA, cB])
                indices.append(contentsOf: [rA, rB, cB])
            }

            // Cap the fingertip
            let tipJoint = Int32(chain.last!)
            let lastL = leftIndices.last!
            let lastR = rightIndices.last!
            indices.append(contentsOf: [lastL, tipJoint, lastR])
        }

        return (vertices, colors, indices, landmark3D)
    }

    // MARK: - Normal Computation

    private func computeNormals(vertices: [Vertex3D], indices: [Int32]) -> [SCNVector3] {
        var normals = [simd_float3](repeating: simd_float3(0, 0, 1), count: vertices.count)

        var i = 0
        while i < indices.count - 2 {
            let i0 = Int(indices[i])
            let i1 = Int(indices[i + 1])
            let i2 = Int(indices[i + 2])
            if i0 < vertices.count && i1 < vertices.count && i2 < vertices.count {
                let p0 = simd_float3(vertices[i0].x, vertices[i0].y, vertices[i0].z)
                let p1 = simd_float3(vertices[i1].x, vertices[i1].y, vertices[i1].z)
                let p2 = simd_float3(vertices[i2].x, vertices[i2].y, vertices[i2].z)
                let n = simd_cross(p1 - p0, p2 - p0)
                let len = simd_length(n)
                if len > 0.00001 {
                    let norm = n / len
                    normals[i0] += norm
                    normals[i1] += norm
                    normals[i2] += norm
                }
            }
            i += 3
        }

        return normals.map {
            let l = simd_length($0)
            let unit = l > 0.0001 ? $0 / l : simd_float3(0, 0, 1)
            return SCNVector3(unit.x, unit.y, unit.z)
        }
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

    // MARK: - Colormap

    private func elevationColor(t: Float) -> (Float, Float, Float) {
        if t < 0.25 {
            let u = t / 0.25
            return (0.1 + 0.05 * u, 0.35 + 0.35 * u, 0.9 - 0.2 * u) // deep blue -> cyan
        } else if t < 0.55 {
            let u = (t - 0.25) / 0.30
            return (0.15 + 0.15 * u, 0.70 + 0.20 * u, 0.70 - 0.35 * u) // cyan -> emerald
        } else if t < 0.80 {
            let u = (t - 0.55) / 0.25
            return (0.30 + 0.65 * u, 0.90 - 0.15 * u, 0.35 - 0.20 * u) // emerald -> gold
        } else {
            let u = (t - 0.80) / 0.20
            return (0.95, 0.75 - 0.55 * u, 0.15 + 0.05 * u) // gold -> crimson
        }
    }
}
