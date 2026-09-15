import PDFKit
import SwiftUI

public struct PediatricReportView: View {
    public let store: CarePlanStore
    public let sessions: [GazeSessionRecord]
    public let mchat: MCHATScoreResult?
    @Environment(\.dismiss) private var dismiss
    @State private var document: PDFDocument?
    @State private var shareURL: URL?

    public init(store: CarePlanStore, sessions: [GazeSessionRecord], mchat: MCHATScoreResult?) {
        self.store = store
        self.sessions = sessions
        self.mchat = mchat
    }

    public var body: some View {
        NavigationStack {
            Group {
                if let document {
                    PDFKitRepresentedView(document: document)
                } else {
                    ContentUnavailableView("Report unavailable", systemImage: "doc.text")
                }
            }
            .navigationTitle("Pediatric report")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
                if let shareURL {
                    ToolbarItem(placement: .primaryAction) {
                        ShareLink(item: shareURL) {
                            Image(systemName: "square.and.arrow.up")
                        }
                    }
                }
            }
            .onAppear { loadReport() }
        }
    }

    private func loadReport() {
        document = build()
        if let document, let data = document.dataRepresentation() {
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("pediatric-report.pdf")
            try? data.write(to: url)
            shareURL = url
        }
    }

    private func build() -> PDFDocument? {
        let profile = ChildProfileStore.shared.profile
        let flagged = store.milestones.filter { !$0.isCompleted && $0.ageMonths <= profile.ageBracket.midpointMonths }
        let note = flagged.prefix(6).map { "\($0.ageMonths)m \($0.domain.shortLabel): \($0.title)" }.joined(separator: "\n")
        let input = DoctorReportInput(
            participantId: ResearchKitStudyStore.shared.fetchEnrollment().participantId,
            childAgeMonths: profile.ageBracket.midpointMonths,
            guardianName: profile.childName.isEmpty ? "Caregiver" : profile.childName,
            gazeRecord: sessions.last,
            mchatResult: mchat,
            careKitCompletionRate: store.completionRatio * 100,
            careKitMedicationsLogged: store.medications.count,
            careKitDiaryEntriesLogged: store.diaryHistory.filter(\.hasCaregiverInput).count,
            notes: note.isEmpty ? "No overdue milestone flags in the current age window." : "Flagged surveillance items:\n\(note)"
        )
        return DoctorReportGenerator.generatePDFDocument(input: input)
    }
}

private struct PDFKitRepresentedView: UIViewRepresentable {
    let document: PDFDocument

    func makeUIView(context: Context) -> PDFView {
        let view = PDFView()
        view.autoScales = true
        view.document = document
        return view
    }

    func updateUIView(_ uiView: PDFView, context: Context) {
        uiView.document = document
    }
}
