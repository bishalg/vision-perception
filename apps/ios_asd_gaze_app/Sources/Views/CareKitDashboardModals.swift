import SwiftUI
import CurvAsdGazeKit

// MARK: - Clinical Details Modal Sheet

public struct ClinicalDetailsModalView: View {
    @Environment(\.dismiss) private var dismiss

    public init() {}

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        HStack(spacing: 12) {
                            ZStack {
                                Circle()
                                    .fill(LiquidGlass.warmAmber.opacity(0.2))
                                    .frame(width: 48, height: 48)
                                Image(systemName: "heart.text.square.fill")
                                    .font(.system(size: 24))
                                    .foregroundColor(LiquidGlass.warmAmber)
                            }

                            VStack(alignment: .leading, spacing: 2) {
                                Text("Clinical Advisory & Protocol")
                                    .font(.system(size: 18, weight: .bold, design: .rounded))
                                    .foregroundColor(.primary)
                                Text("Pediatric Visual Attention Screening")
                                    .font(.system(size: 13))
                                    .foregroundColor(.secondary)
                            }
                        }

                        VStack(alignment: .leading, spacing: 12) {
                            Text("Purpose & Regulatory Scope")
                                .font(.system(size: 15, weight: .bold))
                                .foregroundColor(.primary)

                            Text("This tool measures objective visual attention distributions during standardized social and geometric preference stimuli (GeoPref). It is intended as an adjunctive developmental monitoring instrument and does not provide standalone medical diagnoses.")
                                .font(.system(size: 13))
                                .foregroundColor(.secondary)

                            Divider()

                            Text("Recommended Pediatric Actions")
                                .font(.system(size: 15, weight: .bold))
                                .foregroundColor(.primary)

                            Text("• Share weekly CareKit JSON / PDF export with your child's pediatrician or developmental specialist.\n• Maintain regular daily screenings (30-60 seconds) in a well-lit room.\n• Track behavioral observations and spontaneous eye contact ratings consistently.")
                                .font(.system(size: 13))
                                .foregroundColor(.secondary)
                        }
                        .glassCard(cornerRadius: 14, padding: 16)
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Clinical Advisory")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }
}

// MARK: - Export Modal View

public struct CareKitExportModalView: View {
    @ObservedObject public var store: CarePlanStore
    @Environment(\.dismiss) private var dismiss

    public init(store: CarePlanStore) {
        self.store = store
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        Text("CareKit Pediatric Export Payload")
                            .font(.system(size: 18, weight: .bold))
                            .foregroundColor(.primary)

                        Text("Format complies with Apple CareKit OCKCarePlanActivity & ResearchKit schemas for pediatrician review.")
                            .font(.system(size: 13))
                            .foregroundColor(.secondary)

                        // Payload Viewer
                        ScrollView(.horizontal) {
                            Text(store.exportClinicalCareKitJSON())
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundColor(LiquidGlass.emeraldMint)
                                .padding(12)
                                .background(Color.black.opacity(0.6)) // theme:fixed-media
                                .cornerRadius(8)
                        }

                        ShareLink(
                            item: store.exportClinicalCareKitJSON(),
                            preview: SharePreview("CareKit Clinical Report", image: Image(systemName: "doc.text"))
                        ) {
                            HStack {
                                Spacer()
                                Label("Share Clinical Report", systemImage: "square.and.arrow.up")
                                    .font(.system(size: 15, weight: .semibold))
                                Spacer()
                            }
                            .padding(.vertical, 12)
                            .background(LiquidGlass.celestialTeal)
                            .foregroundColor(.black) // theme:fixed-media
                            .cornerRadius(10)
                        }
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Clinical Export")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }
}

// MARK: - Milestone Detail Modal View

public struct MilestoneDetailModalView: View {
    public let milestone: MilestoneItem
    @Environment(\.dismiss) private var dismiss

    public init(milestone: MilestoneItem) {
        self.milestone = milestone
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                LiquidGlass.deepCanvas.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        HStack {
                            Text(milestone.title)
                                .font(.system(size: 20, weight: .bold, design: .rounded))
                                .foregroundColor(.primary)
                            Spacer()
                            Text(milestone.dueDateString)
                                .font(.system(size: 12))
                                .foregroundColor(milestone.isCompleted ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
                        }

                        Text("CDC / AAP Early Child Behavioral Checkpoints")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(.secondary)

                        VStack(alignment: .leading, spacing: 12) {
                            ForEach(milestone.checklist, id: \.self) { item in
                                HStack(alignment: .top, spacing: 10) {
                                    Image(systemName: milestone.isCompleted ? "checkmark.circle.fill" : "circle")
                                        .foregroundColor(milestone.isCompleted ? LiquidGlass.emeraldMint : LiquidGlass.celestialTeal)
                                        .font(.system(size: 16))

                                    Text(item)
                                        .font(.system(size: 13))
                                        .foregroundColor(.primary)
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                                .padding(.vertical, 4)
                            }
                        }
                        .glassCard(cornerRadius: 14, padding: 14)
                    }
                    .padding(20)
                }
            }
            .navigationTitle("Milestone Detail")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                    .foregroundColor(LiquidGlass.celestialTeal)
                }
            }
        }
    }
}
