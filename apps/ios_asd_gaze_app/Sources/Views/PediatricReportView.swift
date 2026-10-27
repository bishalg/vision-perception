import PDFKit
import SwiftUI

public struct PediatricReportView: View {
    public let store: CarePlanStore
    public let sessions: [GazeSessionRecord]
    public let mchat: MCHATScoreResult?
    @Environment(\.dismiss) private var dismiss
    @State private var document: PDFDocument?
    @State private var shareURL: URL?
    @State private var fhirURL: URL?

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
                if let fhirURL {
                    ToolbarItem(placement: .secondaryAction) {
                        ShareLink(item: fhirURL) {
                            Label("Export Health Record (FHIR)", systemImage: "heart.text.square")
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
        fhirURL = writeFHIRBundle()
    }

    /// FHIR R4 Bundle for the latest exportable session (ADR-0003); simulated or rejected sessions
    /// are never exported (SIM-5, FHIR-2). The subject is the record's pseudonymous token (CNS-7).
    private func writeFHIRBundle() -> URL? {
        guard let record = sessions.last(where: { FHIRBundleBuilder.exportRefusalReason(for: $0) == nil }) else {
            return nil
        }
        let studyStore = ResearchKitStudyStore.shared
        let answered = mchat.map { result in
            (result, FHIRBundleBuilder.mchatAnswers(for: result, in: studyStore.fetchCompletedTaskResults()))
        }
        let subject = FHIRSubject(record: record, ageMonths: ChildProfileStore.shared.profile.ageBracket.midpointMonths)
        let bundle = FHIRBundleBuilder.bundle(for: record, mchat: answered, extraResources: [], subject: subject)
        guard let data = try? FHIRBundleBuilder.jsonData(bundle) else { return nil }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("pediatric-report-fhir.json")
        guard (try? data.write(to: url, options: [.atomic, .completeFileProtection])) != nil else { return nil }
        return url
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
