import SwiftUI

/// Semantic palette and design tokens for the Liquid Glass aesthetic.
public enum LiquidGlass {
    public static let celestialTeal = Color(red: 0.0, green: 0.824, blue: 0.769)
    public static let warmAmber = Color(red: 1.0, green: 0.718, blue: 0.302)
    public static let emeraldMint = Color(red: 0.0, green: 0.902, blue: 0.463)
    public static let deepCanvas = Color(red: 0.04, green: 0.06, blue: 0.10)
    public static let deepCanvasSecondary = Color(red: 0.08, green: 0.11, blue: 0.18)
    public static let cardBackground = Color.white.opacity(0.08)
    public static let cardBorder = Color.white.opacity(0.18)

    public static let defaultCornerRadius: CGFloat = 16.0
}

/// ViewModifier implementing the Liquid Glass card styling.
public struct GlassCardModifier: ViewModifier {
    public var cornerRadius: CGFloat
    public var padding: CGFloat

    public init(cornerRadius: CGFloat = LiquidGlass.defaultCornerRadius, padding: CGFloat = 16.0) {
        self.cornerRadius = cornerRadius
        self.padding = padding
    }

    public func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .fill(.ultraThinMaterial)
            )
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(
                        LinearGradient(
                            colors: [LiquidGlass.cardBorder, Color.white.opacity(0.04)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        ),
                        lineWidth: 1.0
                    )
            )
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            .shadow(color: Color.black.opacity(0.25), radius: 10, x: 0, y: 5)
    }
}

public extension View {
    func glassCard(cornerRadius: CGFloat = LiquidGlass.defaultCornerRadius, padding: CGFloat = 16.0) -> some View {
        modifier(GlassCardModifier(cornerRadius: cornerRadius, padding: padding))
    }
}

/// Frosted Glass Button styled for caregiver actions.
public struct GlassButton: View {
    public let title: String
    public let systemImage: String?
    public let isPrimary: Bool
    public let action: () -> Void

    public init(
        title: String,
        systemImage: String? = nil,
        isPrimary: Bool = true,
        action: @escaping () -> Void
    ) {
        self.title = title
        self.systemImage = systemImage
        self.isPrimary = isPrimary
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if let systemImage {
                    Image(systemName: systemImage)
                        .font(.system(size: 16, weight: .semibold))
                }
                Text(title)
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
            }
            .foregroundColor(isPrimary ? .black : .white)
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
            .frame(maxWidth: .infinity)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(isPrimary ? LiquidGlass.celestialTeal : LiquidGlass.cardBackground)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(isPrimary ? Color.clear : LiquidGlass.cardBorder, lineWidth: 1)
            )
            .shadow(color: isPrimary ? LiquidGlass.celestialTeal.opacity(0.3) : .clear, radius: 8, y: 4)
        }
        .buttonStyle(.plain)
    }
}

/// Trilingual localization dictionary supporting English, Nepali, and Hindi.
public enum AppLocale: String, CaseIterable, Identifiable {
    case english = "en"
    case nepali = "ne"
    case hindi = "hi"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .english: return "English"
        case .nepali: return "नेपाली (Nepali)"
        case .hindi: return "हिन्दी (Hindi)"
        }
    }
}

public enum Localizer {
    private static let strings: [String: [String: String]] = [
        "dashboard_title": [
            "en": "Developmental Gaze Tracking",
            "ne": "बाल विकास दृष्टि अनुगमन",
            "hi": "बाल विकास दृष्टि निगरानी"
        ],
        "dashboard_subtitle": [
            "en": "Pediatric visual attention & milestone screening",
            "ne": "बालबालिकाको सामाजिक दृष्टि र विकास चरण परीक्षण",
            "hi": "बच्चों का सामाजिक दृष्टि और विकास चरण परीक्षण"
        ],
        "start_screening": [
            "en": "Start Screening Session",
            "ne": "परीक्षण सत्र सुरु गर्नुहोस्",
            "hi": "स्क्रीनिंग सत्र शुरू करें"
        ],
        "calibrate_title": [
            "en": "Pop the Bubbles!",
            "ne": "पानीका फोकाहरू फुटाउनुहोस्!",
            "hi": "बुलबुले फोड़ें!"
        ],
        "calibrate_instruction": [
            "en": "Hold device 40-55 cm from child. Look at the bubble to pop it.",
            "ne": "उपकरण बच्चाबाट ४०-५५ सेमि टाढा राख्नुहोस्। फोका हेरेर फुटाउनुहोस्।",
            "hi": "उपकरण बच्चे से 40-55 सेमी दूर रखें। बुलबुले को देखकर फोड़ें।"
        ],
        "scanner_title": [
            "en": "Interactive Gaze Session",
            "ne": "अन्तर्क्रियात्मक दृष्टि सत्र",
            "hi": "इंटरैक्टिव दृष्टि सत्र"
        ],
        "complete_observation": [
            "en": "Complete Observation",
            "ne": "अवलोकन सम्पन्न गर्नुहोस्",
            "hi": "अवलोकन पूरा करें"
        ],
        "side_by_side": [
            "en": "Side-by-Side",
            "ne": "समानान्तर दृश्य",
            "hi": "समानांतर दृश्य"
        ],
        "stimulus_only": [
            "en": "Stimulus Only",
            "ne": "सामग्री मात्र",
            "hi": "केवल सामग्री"
        ],
        "pip_mode": [
            "en": "Picture-in-Picture",
            "ne": "तस्वीर भित्र तस्वीर",
            "hi": "पिक्चर-इन-पिक्चर"
        ],
        "result_title": [
            "en": "Observation Evidence Report",
            "ne": "अवलोकन प्रमाण प्रतिवेदन",
            "hi": "अवलोकन साक्ष्य रिपोर्ट"
        ],
        "biomarkers_title": [
            "en": "Clinical Social Attention Biomarkers",
            "ne": "क्लिनिकल सामाजिक ध्यान बायोमार्करहरू",
            "hi": "क्लिनिकल सामाजिक ध्यान बायोमार्कर्स"
        ],
        "emotion_title": [
            "en": "Facial Affect & Emotion Metrics",
            "ne": "अनुहारको भाव र संवेग सूचकहरू",
            "hi": "चेहरे का भाव और संवेग संकेतक"
        ],
        "export_researchkit": [
            "en": "Export ResearchKit JSON",
            "ne": "ResearchKit JSON निर्यात गर्नुहोस्",
            "hi": "ResearchKit JSON निर्यात करें"
        ],
        "export_pdf": [
            "en": "Export Clinical PDF",
            "ne": "क्लिनिकल PDF निर्यात गर्नुहोस्",
            "hi": "क्लिनिकल PDF निर्यात करें"
        ],
        "retake": [
            "en": "Retake Observation",
            "ne": "पुन: अवलोकन लिनुहोस्",
            "hi": "पुनः अवलोकन लें"
        ]
    ]

    public static func tr(_ key: String, lang: AppLocale = .english) -> String {
        strings[key]?[lang.rawValue] ?? strings[key]?["en"] ?? key
    }
}
