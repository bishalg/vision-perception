import Foundation
import PDFKit
import UIKit

/// Input model for the observation diary report a parent can share with their pediatrician.
public struct DoctorReportInput: Sendable {
    public let participantId: String
    public let childAgeMonths: Int
    public let guardianName: String
    public let assessmentDate: Date
    public let gazeRecord: GazeSessionRecord?
    public let mchatResult: MCHATScoreResult?
    public let careKitCompletionRate: Double?
    public let careKitMedicationsLogged: Int?
    public let careKitDiaryEntriesLogged: Int?
    public let notes: String?

    public init(
        participantId: String,
        childAgeMonths: Int = 24,
        guardianName: String = "Parent / Legal Guardian",
        assessmentDate: Date = Date(),
        gazeRecord: GazeSessionRecord? = nil,
        mchatResult: MCHATScoreResult? = nil,
        careKitCompletionRate: Double? = nil,
        careKitMedicationsLogged: Int? = nil,
        careKitDiaryEntriesLogged: Int? = nil,
        notes: String? = nil
    ) {
        self.participantId = participantId
        self.childAgeMonths = childAgeMonths
        self.guardianName = guardianName
        self.assessmentDate = assessmentDate
        self.gazeRecord = gazeRecord
        self.mchatResult = mchatResult
        self.careKitCompletionRate = careKitCompletionRate
        self.careKitMedicationsLogged = careKitMedicationsLogged
        self.careKitDiaryEntriesLogged = careKitDiaryEntriesLogged
        self.notes = notes
    }
}

/// Renders the shareable observation diary PDF: gaze observations, M-CHAT-R/F answers and
/// CareKit routine adherence. It reports measurements only. It carries the mandatory
/// non-diagnostic sentence and mission line (CLM-5) and a banner for simulated data (SIM-5).
public enum DoctorReportGenerator {

    /// Standard US Letter dimensions (612 x 792 pt)
    public static let pageWidth: CGFloat = 612.0
    public static let pageHeight: CGFloat = 792.0
    public static let margin: CGFloat = 36.0
    public static var contentWidth: CGFloat { pageWidth - (margin * 2) }

    /// Banner drawn when the report is given a simulated session (SIM-5).
    public static let simulatedBanner = "SIMULATED DATA. NOT A REAL OBSERVATION. DO NOT USE FOR MEDICAL DECISIONS."

    /// Accepts both `Double` and `Double?` so the layout code does not depend on whether a
    /// metric is optional in the record (absent metrics render as "Not observed", PLC-2).
    private static func opt(_ value: Double?) -> Double? { value }

    /// Generates raw PDF Data representation of the report.
    public static func generatePDFData(input: DoctorReportInput) -> Data {
        let pageRect = CGRect(x: 0, y: 0, width: pageWidth, height: pageHeight)
        let renderer = UIGraphicsPDFRenderer(bounds: pageRect)

        return renderer.pdfData { context in
            context.beginPage()
            var currentY: CGFloat = margin

            // 1. Header Banner (plus simulated-data banner when applicable)
            currentY = drawHeader(at: currentY, context: context)
            if input.gazeRecord?.captureSource == .simulated {
                currentY = drawSimulatedBanner(at: currentY)
            }
            currentY += 12

            // 2. Child & Observation Details
            currentY = drawDemographics(input: input, at: currentY)
            currentY += 14

            // 3. Zero Raw Video Guarantee Box
            currentY = drawPrivacySeal(at: currentY)
            currentY += 14

            // 4. Gaze observations
            currentY = drawGazeSection(record: input.gazeRecord, at: currentY)
            currentY += 14

            // 5. Per-clip expected-vs-recorded breakdown
            currentY = drawPerVideoMatrixSection(record: input.gazeRecord, at: currentY)
            currentY += 14

            // 6. M-CHAT-R/F questionnaire
            currentY = drawMChatSection(result: input.mchatResult, at: currentY)
            currentY += 14

            // 7. CareKit routine adherence
            currentY = drawCareKitSection(input: input, at: currentY)
            currentY += 14

            // 8. Discussion notes
            currentY = drawRecommendations(input: input, at: currentY)
            currentY += 16

            // 9. Sign-off & mandatory footer
            drawFooter(at: pageHeight - margin - 48)
        }
    }

    /// Generates a PDFKit PDFDocument instance.
    public static func generatePDFDocument(input: DoctorReportInput) -> PDFDocument? {
        let data = generatePDFData(input: input)
        return PDFDocument(data: data)
    }

    // MARK: - Layout Sections

    private static func drawHeader(at y: CGFloat, context: UIGraphicsPDFRendererContext) -> CGFloat {
        var currentY = y

        let clinicAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 9, weight: .semibold),
            .foregroundColor: UIColor.secondaryLabel
        ]
        let clinicText = "EARLYGAZE OBSERVATION DIARY | PART OF OPEN SPECTRUM TRACKING"
        clinicText.draw(at: CGPoint(x: margin, y: currentY), withAttributes: clinicAttr)
        currentY += 14

        let titleAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 17, weight: .bold),
            .foregroundColor: UIColor.label
        ]
        let titleText = "Social Attention Observation Diary: Summary for Your Pediatrician"
        titleText.draw(at: CGPoint(x: margin, y: currentY), withAttributes: titleAttr)
        currentY += 24

        // Divider
        let path = UIBezierPath()
        path.move(to: CGPoint(x: margin, y: currentY))
        path.addLine(to: CGPoint(x: pageWidth - margin, y: currentY))
        path.lineWidth = 1.0
        UIColor.separator.setStroke()
        path.stroke()
        currentY += 4

        return currentY
    }

    private static func drawSimulatedBanner(at y: CGFloat) -> CGFloat {
        let boxRect = CGRect(x: margin, y: y + 4, width: contentWidth, height: 20)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 4)
        UIColor.systemOrange.withAlphaComponent(0.25).setFill()
        path.fill()
        simulatedBanner.draw(at: CGPoint(x: margin + 8, y: y + 8), withAttributes: [
            .font: UIFont.systemFont(ofSize: 9, weight: .bold),
            .foregroundColor: UIColor.label
        ])
        return y + 24
    }

    private static func drawDemographics(input: DoctorReportInput, at y: CGFloat) -> CGFloat {
        let boxRect = CGRect(x: margin, y: y, width: contentWidth, height: 46)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.secondarySystemBackground.setFill()
        path.fill()

        let labelFont = UIFont.systemFont(ofSize: 8.5, weight: .medium)
        let valueFont = UIFont.systemFont(ofSize: 9.5, weight: .bold)

        let colWidth = contentWidth / 4.0
        let dateStr = DateFormatter.localizedString(from: input.assessmentDate, dateStyle: .medium, timeStyle: .none)

        let items: [(String, String)] = [
            ("CHILD", input.participantId),
            ("AGE", "\(input.childAgeMonths) Months"),
            ("PARENT / GUARDIAN", input.guardianName),
            ("OBSERVATION DATE", dateStr)
        ]

        for (idx, item) in items.enumerated() {
            let colX = margin + 10 + (CGFloat(idx) * colWidth)
            item.0.draw(at: CGPoint(x: colX, y: y + 8), withAttributes: [
                .font: labelFont, .foregroundColor: UIColor.secondaryLabel
            ])
            item.1.draw(at: CGPoint(x: colX, y: y + 22), withAttributes: [
                .font: valueFont, .foregroundColor: UIColor.label
            ])
        }

        return y + 46
    }

    private static func drawPrivacySeal(at y: CGFloat) -> CGFloat {
        let boxRect = CGRect(x: margin, y: y, width: contentWidth, height: 34)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.systemGreen.withAlphaComponent(0.08).setFill()
        path.fill()

        let sealAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8.5, weight: .bold),
            .foregroundColor: UIColor.systemGreen
        ]
        "PRIVATE BY DESIGN: NO VIDEO WAS RECORDED".draw(
            at: CGPoint(x: margin + 10, y: y + 6),
            withAttributes: sealAttr
        )

        let bodyAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular),
            .foregroundColor: UIColor.label
        ]
        "No video, image, or audio was stored. Gaze measurements were computed on the device while the child watched.".draw(
            at: CGPoint(x: margin + 10, y: y + 18),
            withAttributes: bodyAttr
        )

        return y + 34
    }

    private static func drawGazeSection(record: GazeSessionRecord?, at y: CGFloat) -> CGFloat {
        let currentY = y + 0
        drawSectionHeading("1. Gaze Observations: Social vs. Geometric Looking", at: currentY)
        let top = currentY + 18

        let boxRect = CGRect(x: margin, y: top, width: contentWidth, height: 68)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.secondarySystemBackground.setFill()
        path.fill()

        guard let rec = record else {
            "No gaze observation has been recorded yet.".draw(
                at: CGPoint(x: margin + 12, y: top + 26),
                withAttributes: [.font: UIFont.systemFont(ofSize: 9.5, weight: .regular), .foregroundColor: UIColor.secondaryLabel]
            )
            return top + 68
        }

        let colW = contentWidth / 4.0
        let prefPercent = Int(rec.socialPreferenceScore * 100)

        func percentText(_ value: Double?) -> String {
            guard let value else { return "Not observed" }
            return "\(Int(value * 100))%"
        }

        let metrics: [(String, String)] = [
            ("SOCIAL PREFERENCE", "\(prefPercent > 0 ? "+" : "")\(prefPercent)%"),
            ("RECORDING QUALITY", rec.qualityVerdict.uppercased()),
            ("EYE LOOKING SHARE", percentText(opt(rec.eyeDwellRatio))),
            ("MOUTH LOOKING SHARE", percentText(opt(rec.mouthDwellRatio)))
        ]

        for (idx, m) in metrics.enumerated() {
            let colX = margin + 12 + (CGFloat(idx) * colW)
            m.0.draw(at: CGPoint(x: colX, y: top + 8), withAttributes: [
                .font: UIFont.systemFont(ofSize: 8, weight: .medium), .foregroundColor: UIColor.secondaryLabel
            ])
            m.1.draw(at: CGPoint(x: colX, y: top + 22), withAttributes: [
                .font: UIFont.systemFont(ofSize: 13, weight: .bold), .foregroundColor: UIColor.label
            ])
        }

        let subAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular),
            .foregroundColor: UIColor.secondaryLabel
        ]
        let latencyText = opt(rec.firstEyeLatencyMs).map { "\(Int($0)) ms" } ?? "Not observed"
        let subText = "First look at eyes: \(latencyText) | Usable gaze samples: \(Int(rec.validFrameRatio * 100))% | Smile reactivity: \(Int(rec.smileReactivityIndex * 100))%"
        subText.draw(at: CGPoint(x: margin + 12, y: top + 48), withAttributes: subAttr)

        return top + 68
    }

    /// Per-clip expected-vs-recorded table (v1.5 records); compact box otherwise.
    private static func drawPerVideoMatrixSection(record: GazeSessionRecord?, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("2. Per-Clip Engagement (Expected vs Recorded)", at: currentY)
        currentY += 18

        guard let rec = record, let perVideo = rec.perVideo, perVideo.contains(where: { $0.hasBaseline }) else {
            let boxRect = CGRect(x: margin, y: currentY, width: contentWidth, height: 40)
            let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
            UIColor.secondarySystemBackground.setFill()
            path.fill()
            "Per-clip breakdown unavailable for this session (older record or single-clip capture).".draw(
                at: CGPoint(x: margin + 12, y: currentY + 16),
                withAttributes: [.font: UIFont.systemFont(ofSize: 9.5, weight: .regular), .foregroundColor: UIColor.secondaryLabel]
            )
            return currentY + 40
        }

        let rowAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 9, weight: .regular),
            .foregroundColor: UIColor.label
        ]
        let dimAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular),
            .foregroundColor: UIColor.secondaryLabel
        ]

        if let composite = rec.perVideoComposite {
            "OVERALL ENGAGEMENT".draw(at: CGPoint(x: margin + 4, y: currentY), withAttributes: dimAttr)
            let pct = Int((composite * 100).rounded())
            "\(pct)/100".draw(at: CGPoint(x: margin + contentWidth - 60, y: currentY - 3), withAttributes: [
                .font: UIFont.systemFont(ofSize: 12, weight: .bold), .foregroundColor: UIColor.label
            ])
            currentY += 16
        }

        for video in perVideo where video.hasBaseline {
            let statusText: String
            let pct = Int((video.subscore * 100).rounded())
            switch video.subscore {
            case ..<0.35: statusText = "Outside age range"
            case ..<0.65: statusText = "Near range edge"
            default: statusText = "Within age range"
            }

            video.title.draw(at: CGPoint(x: margin + 4, y: currentY), withAttributes: rowAttr)
            "\(pct)/100".draw(at: CGPoint(x: margin + contentWidth - 210, y: currentY), withAttributes: [
                .font: UIFont.systemFont(ofSize: 9, weight: .bold), .foregroundColor: UIColor.label
            ])
            statusText.draw(at: CGPoint(x: margin + contentWidth - 160, y: currentY), withAttributes: [
                .font: UIFont.systemFont(ofSize: 9, weight: .semibold), .foregroundColor: UIColor.label
            ])
            if video.lowConfidence {
                "low quality".draw(at: CGPoint(x: margin + contentWidth - 60, y: currentY), withAttributes: dimAttr)
            }
            currentY += 13

            for row in video.rows {
                let name = row.biomarker.replacingOccurrences(of: "_", with: " ")
                let observed: String
                if row.biomarker.contains("latency") {
                    observed = "\(Int(row.observed)) ms"
                } else if row.biomarker == "fixation_count" {
                    observed = String(format: "%.0f", row.observed)
                } else {
                    observed = "\(Int(row.observed * 100))%"
                }
                let suffix = row.imputed ? " (never looked at)" : ""
                "• \(name): \(observed)\(suffix)".draw(at: CGPoint(x: margin + 14, y: currentY), withAttributes: dimAttr)
                currentY += 11
            }
            currentY += 4
        }

        "Age references are provisional and drawn from published literature. They are not a diagnosis.".draw(
            at: CGPoint(x: margin + 4, y: currentY),
            withAttributes: dimAttr
        )
        return currentY + 14
    }

    private static func drawMChatSection(result: MCHATScoreResult?, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("3. M-CHAT-R/F Parent Questionnaire", at: currentY)
        currentY += 18

        let boxRect = CGRect(x: margin, y: currentY, width: contentWidth, height: 60)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.secondarySystemBackground.setFill()
        path.fill()

        guard let mchat = result else {
            "M-CHAT-R/F questionnaire pending parent completion.".draw(
                at: CGPoint(x: margin + 12, y: currentY + 22),
                withAttributes: [.font: UIFont.systemFont(ofSize: 9.5, weight: .regular), .foregroundColor: UIColor.secondaryLabel]
            )
            return currentY + 60
        }

        "QUESTIONNAIRE RESULT".draw(at: CGPoint(x: margin + 12, y: currentY + 8), withAttributes: [
            .font: UIFont.systemFont(ofSize: 8, weight: .medium), .foregroundColor: UIColor.secondaryLabel
        ])
        ClaimsCopy.mchatLine(for: mchat.riskLevel).draw(at: CGPoint(x: margin + 12, y: currentY + 20), withAttributes: [
            .font: UIFont.systemFont(ofSize: 10, weight: .bold), .foregroundColor: UIColor.label
        ])

        let flagStr: String
        if mchat.failedQuestionIds.isEmpty {
            flagStr = "\(mchat.totalFailed) of 20 items flagged. No items to follow up."
        } else {
            flagStr = "\(mchat.totalFailed) of 20 items flagged: \(mchat.failedQuestionIds.map { "#\($0)" }.joined(separator: ", "))"
        }
        flagStr.draw(at: CGPoint(x: margin + 12, y: currentY + 42), withAttributes: [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular), .foregroundColor: UIColor.secondaryLabel
        ])

        return currentY + 60
    }

    private static func drawCareKitSection(input: DoctorReportInput, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("4. Home Routine and Diary (Last 7 Days)", at: currentY)
        currentY += 18

        let boxRect = CGRect(x: margin, y: currentY, width: contentWidth, height: 48)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.secondarySystemBackground.setFill()
        path.fill()

        let rate = input.careKitCompletionRate ?? 100.0
        let meds = input.careKitMedicationsLogged ?? 0
        let diary = input.careKitDiaryEntriesLogged ?? 0

        let colW = contentWidth / 3.0
        let metrics: [(String, String)] = [
            ("7-DAY ROUTINE COMPLETION", "\(Int(rate))%"),
            ("MEDICATIONS LOGGED", "\(meds) Doses"),
            ("DIARY ENTRIES RECORDED", "\(diary) Entries")
        ]

        for (idx, m) in metrics.enumerated() {
            let colX = margin + 12 + (CGFloat(idx) * colW)
            m.0.draw(at: CGPoint(x: colX, y: currentY + 8), withAttributes: [
                .font: UIFont.systemFont(ofSize: 8, weight: .medium), .foregroundColor: UIColor.secondaryLabel
            ])
            m.1.draw(at: CGPoint(x: colX, y: currentY + 22), withAttributes: [
                .font: UIFont.systemFont(ofSize: 12, weight: .bold), .foregroundColor: UIColor.label
            ])
        }

        return currentY + 48
    }

    private static func drawRecommendations(input: DoctorReportInput, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("5. Notes for the Visit", at: currentY)
        currentY += 18

        var text = "This diary records what the child looked at and how the family routine went. Please share it at your next pediatric visit. Only a qualified professional can evaluate a child's development."
        if let level = input.mchatResult?.riskLevel, level != .low {
            text += " The M-CHAT-R/F result above has a follow-up step: please discuss it with your pediatrician."
        }

        let boxRect = CGRect(x: margin, y: currentY, width: contentWidth, height: 50)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.systemBlue.withAlphaComponent(0.06).setFill()
        path.fill()

        let attr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8.5, weight: .medium),
            .foregroundColor: UIColor.label
        ]
        let textRect = CGRect(x: margin + 10, y: currentY + 8, width: contentWidth - 20, height: 36)
        text.draw(in: textRect, withAttributes: attr)

        return currentY + 50
    }

    private static func drawFooter(at y: CGFloat) {
        let line = UIBezierPath()
        line.move(to: CGPoint(x: margin, y: y))
        line.addLine(to: CGPoint(x: pageWidth - margin, y: y))
        line.lineWidth = 0.5
        UIColor.separator.setStroke()
        line.stroke()

        let signAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular),
            .foregroundColor: UIColor.secondaryLabel
        ]
        "Reviewed by: ___________________________    Date: _________________".draw(
            at: CGPoint(x: margin, y: y + 8),
            withAttributes: signAttr
        )

        // Mandatory sentences (CLM-5), each drawn as its own line so they extract verbatim.
        let sentenceAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8.5, weight: .semibold),
            .foregroundColor: UIColor.label
        ]
        ClaimsCopy.notDiagnostic.draw(at: CGPoint(x: margin, y: y + 22), withAttributes: sentenceAttr)
        ClaimsCopy.mission.draw(at: CGPoint(x: margin, y: y + 34), withAttributes: [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular),
            .foregroundColor: UIColor.secondaryLabel
        ])
    }

    private static func drawSectionHeading(_ text: String, at y: CGFloat) {
        let attr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 10, weight: .bold),
            .foregroundColor: UIColor.label
        ]
        text.draw(at: CGPoint(x: margin, y: y), withAttributes: attr)
    }
}

// MARK: - FHIR R4 DocumentReference (STAGE 2)

extension DoctorReportInput {
    /// Converts this doctor report input into an HL7 FHIR Release 4 `DocumentReference` resource dictionary.
    /// Used for uploading the generated PDF to EHR systems via SMART on FHIR.
    public func toFHIRDocumentReference(
        pdfData: Data,
        participantId: String,
        authorPractitionerId: String? = nil
    ) -> [String: AnyCodableValue] {
        let timestamp = ISO8601DateFormatter().string(from: assessmentDate)
        let base64 = pdfData.base64EncodedString()

        let authorArray: [AnyCodableValue] = authorPractitionerId.map {
            [.dictionary(["reference": .string("Practitioner/\($0)")])]
        } ?? []

        return [
            "resourceType": .string("DocumentReference"),
            "id": .string("doc-pediatric-report-\(participantId)-\(UUID().uuidString.prefix(8))"),
            "status": .string("current"),
            "type": .dictionary([
                "coding": .array([
                    .dictionary([
                        "system": .string("http://loinc.org"),
                        "code": .string("34108-1"),
                        "display": .string("Outpatient note")
                    ])
                ])
            ]),
            "category": .array([
                .dictionary([
                    "coding": .array([
                        .dictionary([
                            "system": .string("http://hl7.org/fhir/us/core/CodeSystem/us-core-documentreference-category"),
                            "code": .string("clinical-note"),
                            "display": .string("Clinical Note")
                        ])
                    ])
                ])
            ]),
            "subject": .dictionary([
                "reference": .string("Patient/\(participantId)"),
                "display": .string("Child (pseudonymous)")
            ]),
            "date": .string(timestamp),
            "author": .array(authorArray),
            "description": .string("EarlyGaze Observation Diary report"),
            "content": .array([
                .dictionary([
                    "attachment": .dictionary([
                        "contentType": .string("application/pdf"),
                        "language": .string("en"),
                        "data": .string(base64),
                        "title": .string("EarlyGaze Observation Diary report")
                    ]),
                    "format": .dictionary([
                        "system": .string("http://ihe.net/fhir/ValueSet/IHE.FormatCode.codesystem"),
                        "code": .string("urn:ihe:iti:xfdf:2008"),
                        "display": .string("PDF Document")
                    ])
                ])
            ]),
            "context": .dictionary([
                "event": .array([
                    .dictionary([
                        "coding": .array([
                            .dictionary([
                                "system": .string("http://snomed.info/sct"),
                                "code": .string("713444003"),
                                "display": .string("Eye tracking examination (procedure)")
                            ])
                        ])
                    ])
                ])
            ])
        ]
    }
}
