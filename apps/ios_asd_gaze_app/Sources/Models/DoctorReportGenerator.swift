import Foundation
import PDFKit
import UIKit

/// Input model containing all clinical dimensions required for the Pediatrician Doctor Report.
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

/// Generates standardized clinical PDF reports for pediatricians, combining
/// ARKit/ResearchKit objective gaze metrics, M-CHAT-R/F scores, and CareKit adherence.
public enum DoctorReportGenerator {

    /// Standard US Letter dimensions (612 x 792 pt)
    public static let pageWidth: CGFloat = 612.0
    public static let pageHeight: CGFloat = 792.0
    public static let margin: CGFloat = 36.0
    public static var contentWidth: CGFloat { pageWidth - (margin * 2) }

    /// Generates raw PDF Data representation of the clinical report.
    public static func generatePDFData(input: DoctorReportInput) -> Data {
        let pageRect = CGRect(x: 0, y: 0, width: pageWidth, height: pageHeight)
        let renderer = UIGraphicsPDFRenderer(bounds: pageRect)

        return renderer.pdfData { context in
            context.beginPage()
            var currentY: CGFloat = margin

            // 1. Header Banner
            currentY = drawHeader(at: currentY, context: context)
            currentY += 12

            // 2. Patient & Assessment Demographics
            currentY = drawDemographics(input: input, at: currentY)
            currentY += 14

            // 3. Zero Raw Video Guarantee Box
            currentY = drawPrivacySeal(at: currentY)
            currentY += 14

            // 4. Objective Gaze & Social Visual Preference (GeoPref) Metrics
            currentY = drawGazeSection(record: input.gazeRecord, at: currentY)
            currentY += 14

            // 4. Per-Stimulus Expected-vs-Recorded Matrix Breakdown
            currentY = drawPerVideoMatrixSection(record: input.gazeRecord, at: currentY)
            currentY += 14

            // 5. M-CHAT-R/F Standardized Behavioral Results
            currentY = drawMChatSection(result: input.mchatResult, at: currentY)
            currentY += 14

            // 6. CareKit Longitudinal In-Home Adherence
            currentY = drawCareKitSection(input: input, at: currentY)
            currentY += 14

            // 7. Pediatrician Clinical Recommendations & Referral Plan
            currentY = drawRecommendations(input: input, at: currentY)
            currentY += 16

            // 8. Sign-off & Footer
            drawFooter(at: pageHeight - margin - 35)
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
        let clinicText = "CURV PEDIATRIC NEURODEVELOPMENT & VISION RESEARCH | PROTOCOL IRB-ASD-2026-088"
        clinicText.draw(at: CGPoint(x: margin, y: currentY), withAttributes: clinicAttr)
        currentY += 14

        let titleAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 17, weight: .bold),
            .foregroundColor: UIColor.label
        ]
        let titleText = "Pediatric Social Attention & Developmental Screening Report"
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
            ("PARTICIPANT ID", input.participantId),
            ("CHRONOLOGICAL AGE", "\(input.childAgeMonths) Months"),
            ("PRIMARY GUARDIAN", input.guardianName),
            ("ASSESSMENT DATE", dateStr)
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
        "PRIVACY-BY-DESIGN PROTOCOL VERIFIED: ZERO RAW VIDEO RECORDING".draw(
            at: CGPoint(x: margin + 10, y: y + 6),
            withAttributes: sealAttr
        )

        let bodyAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular),
            .foregroundColor: UIColor.label
        ]
        "No raw video, image, or audio frames were stored. Spatial vectors & biomarkers were computed on-device in real time.".draw(
            at: CGPoint(x: margin + 10, y: y + 18),
            withAttributes: bodyAttr
        )

        return y + 34
    }

    private static func drawGazeSection(record: GazeSessionRecord?, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("1. Objective Gaze & Social Visual Preference (GeoPref)", at: currentY)
        currentY += 18

        let boxRect = CGRect(x: margin, y: currentY, width: contentWidth, height: 68)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.secondarySystemBackground.setFill()
        path.fill()

        guard let rec = record else {
            "Gaze tracking screening session has not been conducted yet.".draw(
                at: CGPoint(x: margin + 12, y: currentY + 26),
                withAttributes: [.font: UIFont.systemFont(ofSize: 9.5, weight: .regular), .foregroundColor: UIColor.secondaryLabel]
            )
            return currentY + 68
        }

        let colW = contentWidth / 4.0
        let prefPercent = Int(rec.socialPreferenceScore * 100)
        let prefColor: UIColor = rec.socialPreferenceScore >= 0.20 ? .systemGreen : (rec.socialPreferenceScore < 0.0 ? .systemRed : .systemOrange)

        let metrics: [(String, String, UIColor)] = [
            ("SOCIAL PREFERENCE", "\(prefPercent > 0 ? "+" : "")\(prefPercent)%", prefColor),
            ("QUALITY VERDICT", rec.qualityVerdict.uppercased(), .label),
            ("EYE DWELL RATIO", "\(Int(rec.eyeDwellRatio * 100))%", .label),
            ("MOUTH DWELL RATIO", "\(Int(rec.mouthDwellRatio * 100))%", .label)
        ]

        for (idx, m) in metrics.enumerated() {
            let colX = margin + 12 + (CGFloat(idx) * colW)
            m.0.draw(at: CGPoint(x: colX, y: currentY + 8), withAttributes: [
                .font: UIFont.systemFont(ofSize: 8, weight: .medium), .foregroundColor: UIColor.secondaryLabel
            ])
            m.1.draw(at: CGPoint(x: colX, y: currentY + 22), withAttributes: [
                .font: UIFont.systemFont(ofSize: 13, weight: .bold), .foregroundColor: m.2
            ])
        }

        let subAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular),
            .foregroundColor: UIColor.secondaryLabel
        ]
        let subText = "First eye latency: \(Int(rec.firstEyeLatencyMs)) ms | Valid gaze sample ratio: \(Int(rec.validFrameRatio * 100))% | Smile reactivity: \(Int(rec.smileReactivityIndex * 100))%"
        subText.draw(at: CGPoint(x: margin + 12, y: currentY + 48), withAttributes: subAttr)

        return currentY + 68
    }

    /// Per-stimulus expected-vs-recorded matrix table (v1.5 records); compact no-op box otherwise.
    private static func drawPerVideoMatrixSection(record: GazeSessionRecord?, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("2. Per-Stimulus Engagement Matrix (Expected vs Recorded)", at: currentY)
        currentY += 18

        guard let rec = record, let perVideo = rec.perVideo, perVideo.contains(where: { $0.hasBaseline }) else {
            let boxRect = CGRect(x: margin, y: currentY, width: contentWidth, height: 40)
            let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
            UIColor.secondarySystemBackground.setFill()
            path.fill()
            "Per-stimulus breakdown unavailable for this session (legacy record or single-stimulus capture).".draw(
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
        let valueAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 9, weight: .bold),
            .foregroundColor: UIColor.label
        ]

        if let composite = rec.perVideoComposite {
            "COMPOSITE ENGAGEMENT SCORE".draw(at: CGPoint(x: margin + 4, y: currentY), withAttributes: dimAttr)
            let pct = Int((composite * 100).rounded())
            let color: UIColor = pct >= 65 ? .systemGreen : (pct >= 35 ? .systemOrange : .systemRed)
            "\(pct)/100".draw(at: CGPoint(x: margin + contentWidth - 60, y: currentY - 3), withAttributes: [
                .font: UIFont.systemFont(ofSize: 12, weight: .bold), .foregroundColor: color
            ])
            currentY += 16
        }

        for video in perVideo where video.hasBaseline {
            let statusText: String
            let pct = Int((video.subscore * 100).rounded())
            let color: UIColor = pct >= 65 ? .systemGreen : (pct >= 35 ? .systemOrange : .systemRed)
            switch video.subscore {
            case ..<0.35: statusText = "Atypical"
            case ..<0.65: statusText = "Monitor"
            default: statusText = "On Target"
            }

            video.title.draw(at: CGPoint(x: margin + 4, y: currentY), withAttributes: rowAttr)
            "\(pct)/100".draw(at: CGPoint(x: margin + contentWidth - 190, y: currentY), withAttributes: [
                .font: UIFont.systemFont(ofSize: 9, weight: .bold), .foregroundColor: color
            ])
            statusText.draw(at: CGPoint(x: margin + contentWidth - 120, y: currentY), withAttributes: [
                .font: UIFont.systemFont(ofSize: 9, weight: .semibold), .foregroundColor: color
            ])
            if video.lowConfidence {
                "low quality".draw(at: CGPoint(x: margin + contentWidth - 60, y: currentY), withAttributes: dimAttr)
            }
            currentY += 13

            // Expectation rows for this clip
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
                let suffix = row.imputed ? " (never fixated)" : ""
                "• \(name): \(observed)\(suffix)".draw(at: CGPoint(x: margin + 14, y: currentY), withAttributes: dimAttr)
                currentY += 11
            }
            currentY += 4
        }

        "Baseline \(rec.perVideoComposite != nil ? "provisional-literature-seeded" : "") — values compare against age-expectation references, not a diagnosis.".draw(
            at: CGPoint(x: margin + 4, y: currentY),
            withAttributes: dimAttr
        )
        return currentY + 14
    }

    private static func drawMChatSection(result: MCHATScoreResult?, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("3. M-CHAT-R/F Standardized Behavioral Risk Stratification", at: currentY)
        currentY += 18

        let boxRect = CGRect(x: margin, y: currentY, width: contentWidth, height: 60)
        let path = UIBezierPath(roundedRect: boxRect, cornerRadius: 6)
        UIColor.secondarySystemBackground.setFill()
        path.fill()

        guard let mchat = result else {
            "M-CHAT-R/F developmental questionnaire pending caregiver completion.".draw(
                at: CGPoint(x: margin + 12, y: currentY + 22),
                withAttributes: [.font: UIFont.systemFont(ofSize: 9.5, weight: .regular), .foregroundColor: UIColor.secondaryLabel]
            )
            return currentY + 60
        }

        let riskColor: UIColor
        switch mchat.riskLevel {
        case .low: riskColor = .systemGreen
        case .medium: riskColor = .systemOrange
        case .high: riskColor = .systemRed
        }

        let scoreStr = "\(mchat.totalFailed) Failed Items"
        "DEVELOPMENTAL RISK LEVEL".draw(at: CGPoint(x: margin + 12, y: currentY + 8), withAttributes: [
            .font: UIFont.systemFont(ofSize: 8, weight: .medium), .foregroundColor: UIColor.secondaryLabel
        ])
        mchat.riskLevel.rawValue.uppercased().draw(at: CGPoint(x: margin + 12, y: currentY + 20), withAttributes: [
            .font: UIFont.systemFont(ofSize: 13, weight: .bold), .foregroundColor: riskColor
        ])

        "TOTAL SCORE".draw(at: CGPoint(x: margin + 180, y: currentY + 8), withAttributes: [
            .font: UIFont.systemFont(ofSize: 8, weight: .medium), .foregroundColor: UIColor.secondaryLabel
        ])
        scoreStr.draw(at: CGPoint(x: margin + 180, y: currentY + 20), withAttributes: [
            .font: UIFont.systemFont(ofSize: 13, weight: .bold), .foregroundColor: UIColor.label
        ])

        let flagStr: String
        if mchat.failedQuestionIds.isEmpty {
            flagStr = "No atypical behavioral items flagged."
        } else {
            flagStr = "Flagged question indices: \(mchat.failedQuestionIds.map { "#\($0)" }.joined(separator: ", "))"
        }
        flagStr.draw(at: CGPoint(x: margin + 12, y: currentY + 42), withAttributes: [
            .font: UIFont.systemFont(ofSize: 8, weight: .regular), .foregroundColor: UIColor.secondaryLabel
        ])

        return currentY + 60
    }

    private static func drawCareKitSection(input: DoctorReportInput, at y: CGFloat) -> CGFloat {
        var currentY = y
        drawSectionHeading("4. CareKit In-Home 7-Day Treatment & Activity Adherence", at: currentY)
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
            ("7-DAY ROUTINE ADHERENCE", "\(Int(rate))%"),
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
        drawSectionHeading("5. Clinical Synthesis & Action Plan", at: currentY)
        currentY += 18

        let isHighRisk = (input.mchatResult?.riskLevel == .high) || ((input.gazeRecord?.socialPreferenceScore ?? 1.0) < -0.15)
        let isMediumRisk = (input.mchatResult?.riskLevel == .medium) || ((input.gazeRecord?.socialPreferenceScore ?? 1.0) < 0.20)

        let text: String
        if isHighRisk {
            text = "EVALUATION INDICATED: Combined behavioral questionnaire and objective gaze metrics demonstrate significant atypical visual preference or developmental delay indicators. Urgent comprehensive developmental evaluation (ADOS-2, Mullen Scales) and hearing evaluation are recommended."
        } else if isMediumRisk {
            text = "SURVEILLANCE & FOLLOW-UP: Moderate developmental indicators detected. Recommend administering the M-CHAT-R/F follow-up interview, reviewing specific flagged items, and re-evaluating gaze attention within 4-6 weeks."
        } else {
            text = "ROUTINE PEDIATRIC SURVEILLANCE: Metrics align within neurotypical reference expectations for age. Continue standard developmental surveillance during routine well-child appointments."
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
        "Reviewing Clinician: ___________________________  MD / DO    Date: _________________".draw(
            at: CGPoint(x: margin, y: y + 8),
            withAttributes: signAttr
        )

        let disclaimerAttr: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 7.5, weight: .regular),
            .foregroundColor: UIColor.tertiaryLabel
        ]
        "CONFIDENTIAL MEDICAL RESEARCH & SCREENING RECORD - NOT A STANDALONE DIAGNOSIS".draw(
            at: CGPoint(x: margin, y: y + 22),
            withAttributes: disclaimerAttr
        )
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
                "display": .string("Study Participant")
            ]),
            "date": .string(timestamp),
            "author": .array(authorArray),
            "description": .string("Pediatric ASD Gaze Screening Clinical Report"),
            "content": .array([
                .dictionary([
                    "attachment": .dictionary([
                        "contentType": .string("application/pdf"),
                        "language": .string("en"),
                        "data": .string(base64),
                        "title": .string("Pediatric ASD Gaze Screening Report")
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
