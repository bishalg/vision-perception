import SwiftUI
import UIKit

/// Semantic palette and design tokens for the Liquid Glass aesthetic.
///
/// The canvas + card tokens are *adaptive*: deep twilight surfaces in dark
/// mode, airy light surfaces in light mode, so every screen follows the
/// system theme on launch. Accent hues stay identical in both modes.
/// Text must use `.primary` / `.secondary` (never fixed white) on any
/// surface painted from these tokens. Fixed white is reserved for media
/// contrast treatments directly over video, photo, or stimulus fields.
public enum LiquidGlass {
    public static let celestialTeal = Color(red: 0.0, green: 0.824, blue: 0.769)
    public static let warmAmber = Color(red: 1.0, green: 0.718, blue: 0.302)
    public static let emeraldMint = Color(red: 0.0, green: 0.902, blue: 0.463)
    public static let deepCanvas = adaptive(
        dark: Color(red: 0.04, green: 0.06, blue: 0.10),
        light: Color(red: 0.95, green: 0.96, blue: 0.98)
    )
    public static let deepCanvasSecondary = adaptive(
        dark: Color(red: 0.08, green: 0.11, blue: 0.18),
        light: Color(red: 0.88, green: 0.91, blue: 0.95)
    )
    public static let cardBackground = adaptive(
        dark: Color.white.opacity(0.08),
        light: Color.black.opacity(0.05)
    )
    public static let cardBorder = adaptive(
        dark: Color.white.opacity(0.18),
        light: Color.black.opacity(0.12)
    )

    // MARK: - Central Typography & Text Colors (Single Source of Truth)
    public static let textPrimary = adaptive(
        dark: Color.white.opacity(0.96),
        light: Color(red: 0.08, green: 0.10, blue: 0.14)
    )
    public static let textSecondary = adaptive(
        dark: Color.white.opacity(0.65),
        light: Color(red: 0.38, green: 0.42, blue: 0.48)
    )
    public static let textTertiary = adaptive(
        dark: Color.white.opacity(0.40),
        light: Color(red: 0.55, green: 0.58, blue: 0.63)
    )
    public static let textHeadline = adaptive(
        dark: Color.white.opacity(0.92),
        light: Color(red: 0.06, green: 0.08, blue: 0.12)
    )
    public static let dividerColor = adaptive(
        dark: Color.white.opacity(0.12),
        light: Color.black.opacity(0.08)
    )

    // MARK: - Adaptive Stimulus Panes (Clinical Observation)
    public static let stimulusSocialTop = adaptive(
        dark: Color(red: 0.10, green: 0.16, blue: 0.28),
        light: Color(red: 0.88, green: 0.93, blue: 0.99)
    )
    public static let stimulusSocialBottom = adaptive(
        dark: Color(red: 0.05, green: 0.08, blue: 0.16),
        light: Color(red: 0.80, green: 0.88, blue: 0.97)
    )
    public static let stimulusGeometryTop = adaptive(
        dark: Color(red: 0.20, green: 0.08, blue: 0.24),
        light: Color(red: 0.95, green: 0.90, blue: 0.98)
    )
    public static let stimulusGeometryBottom = adaptive(
        dark: Color(red: 0.08, green: 0.04, blue: 0.12),
        light: Color(red: 0.88, green: 0.81, blue: 0.94)
    )

    public static let defaultCornerRadius: CGFloat = 16.0
}

/// Resolves per-environment so screens follow the system theme on launch.
private func adaptive(dark: Color, light: Color) -> Color {
    Color(UIColor { trait in
        trait.userInterfaceStyle == .dark ? UIColor(dark) : UIColor(light)
    })
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
            .foregroundColor(isPrimary ? .black : .primary)
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

// MARK: - Single Source of Truth Typography Components

/// Standardized typography scale ensuring complete light and dark theme legibility.
public enum GlassTextStyle {
    case largeTitle     // 28pt bold rounded
    case title          // 20pt bold rounded
    case sectionHeader  // 14pt bold rounded
    case headline       // 14pt semibold
    case body           // 13pt regular
    case subheadline    // 12pt medium
    case caption        // 11pt regular
    case metricNumber   // 24pt bold rounded
}

public struct GlassTextModifier: ViewModifier {
    public let style: GlassTextStyle
    public let color: Color?

    public func body(content: Content) -> some View {
        switch style {
        case .largeTitle:
            content
                .font(.system(size: 28, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .title:
            content
                .font(.system(size: 20, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .sectionHeader:
            content
                .font(.system(size: 14, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textHeadline)
        case .headline:
            content
                .font(.system(size: 14, weight: .semibold, design: .default))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .body:
            content
                .font(.system(size: 13, weight: .regular, design: .default))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        case .subheadline:
            content
                .font(.system(size: 12, weight: .medium, design: .default))
                .foregroundColor(color ?? LiquidGlass.textSecondary)
        case .caption:
            content
                .font(.system(size: 11, weight: .regular, design: .default))
                .foregroundColor(color ?? LiquidGlass.textSecondary)
        case .metricNumber:
            content
                .font(.system(size: 24, weight: .bold, design: .rounded))
                .foregroundColor(color ?? LiquidGlass.textPrimary)
        }
    }
}

public extension View {
    func glassStyle(_ style: GlassTextStyle, color: Color? = nil) -> some View {
        modifier(GlassTextModifier(style: style, color: color))
    }
}

/// Centralized Section Header adhering strictly to Liquid Glass & Apple HIG themes.
/// Automatically adapts icon and typography for flawless contrast in both light and dark mode.
public struct GlassSectionHeader: View {
    public let title: String
    public let icon: String?
    public var iconColor: Color

    public init(title: String, icon: String? = nil, iconColor: Color = LiquidGlass.celestialTeal) {
        self.title = title
        self.icon = icon
        self.iconColor = iconColor
    }

    public var body: some View {
        HStack(spacing: 6) {
            if let icon {
                Image(systemName: icon)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(iconColor)
            }
            Text(title)
                .glassStyle(.sectionHeader)
        }
        .padding(.horizontal, 4)
    }
}

/// Theme-adaptive subtle Divider.
public struct GlassDivider: View {
    public init() {}

    public var body: some View {
        Divider()
            .overlay(LiquidGlass.dividerColor)
    }
}

/// Standardized hardware / sensor diagnostic row adhering to light and dark theme contrast.
public struct GlassDiagnosticRow: View {
    public let title: String
    public let subtitle: String
    public let statusText: String
    public let isGood: Bool

    public init(title: String, subtitle: String, statusText: String, isGood: Bool) {
        self.title = title
        self.subtitle = subtitle
        self.statusText = statusText
        self.isGood = isGood
    }

    public var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .glassStyle(.headline)
                Text(subtitle)
                    .glassStyle(.caption)
            }
            Spacer()
            Text(statusText)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(Capsule().fill(isGood ? LiquidGlass.emeraldMint.opacity(0.18) : LiquidGlass.warmAmber.opacity(0.18)))
                .foregroundColor(isGood ? LiquidGlass.emeraldMint : LiquidGlass.warmAmber)
        }
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

    /// Automatically detects user's preferred language from iOS Settings.
    public static var current: AppLocale {
        let preferred = Locale.preferredLanguages.first?.prefix(2).lowercased() ?? Locale.current.language.languageCode?.identifier.prefix(2).lowercased() ?? "en"
        if preferred.hasPrefix("ne") {
            return .nepali
        } else if preferred.hasPrefix("hi") {
            return .hindi
        }
        return .english
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
            "en": "Pop the Bubbles",
            "ne": "पानीका फोकाहरू फुटाउनुहोस्",
            "hi": "बुलबुले फोड़ें"
        ],
        "calibrate_instruction": [
            "en": "Hold device 40-50 cm away. Look at the balloon to pop it.",
            "ne": "उपकरण ४०-५० सेमि टाढा राख्नुहोस्। बेलुन हेरेर फुटाउनुहोस्।",
            "hi": "उपकरण 40-50 सेमी दूर रखें। गुब्बारा देखकर फोड़ें।"
        ],
        "dev_gaze_title": [
            "en": "Developer Gaze Tracking",
            "ne": "विकासकर्ता दृष्टि अनुगमन",
            "hi": "डेवलपर दृष्टि ट्रैकिंग"
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
        ],
        "close": [
            "en": "Close",
            "ne": "बन्द गर्नुहोस्",
            "hi": "बंद करें"
        ],
        "skip": [
            "en": "Skip",
            "ne": "छोड्नुहोस्",
            "hi": "छोड़ें"
        ],
        "next": [
            "en": "Next Target",
            "ne": "अर्को लक्ष्य",
            "hi": "अगला लक्ष्य"
        ],
        "distance_optimal": [
            "en": "Optimal Distance (40–50 cm)",
            "ne": "उत्तम दूरी (४०-५० सेमि)",
            "hi": "इष्टतम दूरी (40-50 सेमी)"
        ],
        "distance_too_close": [
            "en": "Too Close (Move Back)",
            "ne": "धेरै नजिक (पछाडि सर्नुहोस्)",
            "hi": "बहुत पास (पीछे हटें)"
        ],
        "distance_too_far": [
            "en": "Too Far (Move Closer)",
            "ne": "धेरै टाढा (नजिक ल्याउनुहोस्)",
            "hi": "बहुत दूर (पास लाएं)"
        ],
        "eye_tracked_active": [
            "en": "Tracked",
            "ne": "ट्र्याक भयो",
            "hi": "ट्रैक हुई"
        ],
        "eye_searching": [
            "en": "Align Child's Face",
            "ne": "बच्चाको अनुहार मिलाउनुहोस्",
            "hi": "बच्चे का चेहरा संरेखित करें"
        ],
        "tracking_locked": [
            "en": "Tracking Locked! Keep looking...",
            "ne": "ट्र्याकिङ लक भयो! हेरिरहनुहोस्...",
            "hi": "ट्रैकिंग लॉक! देखते रहें..."
        ],
        "tap_to_pop_hint": [
            "en": "Look at the balloon to pop it • Hands-Free",
            "ne": "बेलुन हेरेर फुटाउनुहोस् • हात नछोई",
            "hi": "गुब्बारा देखकर फोड़ें • हाथों के बिना"
        ],
        "hands_free_hint": [
            "en": "Look at the balloon to pop it • Hands-Free",
            "ne": "बेलुन हेरेर फुटाउनुहोस् • हात नछोई",
            "hi": "गुब्बारा देखकर फोड़ें • हाथों के बिना"
        ],
        "popping_locked_hint": [
            "en": "Inflating balloon... Keep looking! ✨",
            "ne": "बेलुन फुल्दै छ... हेरिरहनुहोस्! ✨",
            "hi": "गुब्बारा फूल रहा है... देखते रहें! ✨"
        ],
        "tab_dashboard": [
            "en": "Dashboard",
            "ne": "ड्यासबोर्ड",
            "hi": "डैशबोर्ड"
        ],
        "tab_gaze": [
            "en": "Gaze Tracker",
            "ne": "दृष्टि ट्र्याकर",
            "hi": "दृष्टि ट्रैकर"
        ],
        "tab_calibrate": [
            "en": "Calibration",
            "ne": "क्यालिब्रेसन",
            "hi": "कैलिब्रेशन"
        ],
        "tab_settings": [
            "en": "Settings",
            "ne": "सेटिङहरू",
            "hi": "सेटिंग्स"
        ],
        "carekit_today_plan": [
            "en": "Today's Care Plan",
            "ne": "आजको हेरचाह योजना",
            "hi": "आज की देखभाल योजना"
        ],
        "carekit_daily_habits": [
            "en": "Daily Habits & Interventions",
            "ne": "दैनिक बानी र अन्तर्क्रिया",
            "hi": "दैनिक आदतें और बातचीत"
        ],
        "carekit_ring_completed": [
            "en": "Completed Today",
            "ne": "आज सम्पन्न भयो",
            "hi": "आज पूरा हुआ"
        ],
        "carekit_diary_title": [
            "en": "Daily Sensory & Mood Diary",
            "ne": "दैनिक संवेदी र मुड डायरी",
            "hi": "दैनिक संवेदी और मूड डायरी"
        ],
        "carekit_diary_notes_placeholder": [
            "en": "Notes on eye contact, gestures, or words today...",
            "ne": "आजको दृष्टि सम्पर्क, संकेत वा शब्दहरूको टिप्पणी...",
            "hi": "आज के दृष्टि संपर्क, इशारों या शब्दों के नोट्स..."
        ],
        "carekit_trend_title": [
            "en": "7-Day Social Attention Trend",
            "ne": "७-दिने सामाजिक ध्यान प्रवृत्ति",
            "hi": "7-दिवसीय सामाजिक ध्यान प्रवृत्ति"
        ],
        "carekit_milestones_title": [
            "en": "CDC / AAP Developmental Milestones",
            "ne": "विकास चरण कोशेढुङ्गाहरू",
            "hi": "विकासात्मक मील के पत्थर"
        ],
        "carekit_export_title": [
            "en": "Export CareKit Clinical Summary",
            "ne": "CareKit क्लिनिकल प्रतिवेदन निर्यात गर्नुहोस्",
            "hi": "CareKit क्लिनिकल सारांश निर्यात करें"
        ],
        "carekit_streak": [
            "en": "Day Streak",
            "ne": "दिनको निरन्तरता",
            "hi": "दिवसीय निरंतरता"
        ],
        "dashboard_pinned": [
            "en": "Pinned",
            "ne": "पिन गरिएका",
            "hi": "पिन किए गए"
        ],
        "dashboard_clinical_advisory": [
            "en": "CLINICAL ADVISORY",
            "ne": "क्लिनिकल सल्लाह",
            "hi": "क्लिनिकल सलाह"
        ],
        "dashboard_clinical_title": [
            "en": "Objective Visual Attention Tracking",
            "ne": "वस्तुनिष्ठ सामाजिक दृष्टि अनुगमन",
            "hi": "वस्तुनिष्ठ सामाजिक दृष्टि ट्रैकिंग"
        ],
        "dashboard_clinical_desc": [
            "en": "Measures objective visual attention distributions during standardized stimuli. Does not provide standalone diagnostic determinations. Consult a developmental pediatrician.",
            "ne": "मानकीकृत सामग्रीहरूमा दृश्य ध्यान वितरण मापन गर्दछ। यसले एक्लै निदान प्रदान गर्दैन। बाल रोग विशेषज्ञसँग परामर्श लिनुहोस्।",
            "hi": "मानकीकृत सामग्री के दौरान दृश्य ध्यान वितरण को मापता है। यह अकेले निदान प्रदान नहीं करता है। बाल रोग विशेषज्ञ से परामर्श लें।"
        ],
        "dashboard_more_details": [
            "en": "More Details",
            "ne": "थप विवरण",
            "hi": "और विवरण"
        ],
        "dashboard_care_activity": [
            "en": "Care Protocol Activity",
            "ne": "हेरचाह प्रोटोकल गतिविधि",
            "hi": "देखभाल प्रोटोकॉल गतिविधि"
        ],
        "dashboard_social_focus": [
            "en": "Social Focus",
            "ne": "सामाजिक ध्यान",
            "hi": "सामाजिक ध्यान"
        ],
        "dashboard_child_state": [
            "en": "Child's Engagement & State Today",
            "ne": "आज बच्चाको सहभागिता र अवस्था",
            "hi": "आज बच्चे की सहभागिता और स्थिति"
        ],
        "dashboard_eye_contact": [
            "en": "Spontaneous Eye Contact",
            "ne": "स्वतःस्फूर्त दृष्टि सम्पर्क",
            "hi": "सहज दृष्टि संपर्क"
        ],
        "dashboard_highlights": [
            "en": "Highlights",
            "ne": "मुख्य हाइलाइटहरू",
            "hi": "मुख्य हाइलाइट्स"
        ],
        "dashboard_highlight_note": [
            "en": "On average, this week's social visual attention is consistent with typical developmental milestones.",
            "ne": "औसतमा, यस हप्ताको सामाजिक दृष्टि ध्यान सामान्य विकासात्मक कोशेढुङ्गाहरूसँग मेल खान्छ।",
            "hi": "औसत रूप से, इस सप्ताह का सामाजिक दृष्टि ध्यान सामान्य विकासात्मक मील के पत्थरों के अनुरूप है।"
        ],
        "settings_language_section": [
            "en": "Language & Localization",
            "ne": "भाषा र स्थानीयकरण",
            "hi": "भाषा और स्थानीयकरण"
        ],
        "settings_active_language": [
            "en": "Active Language",
            "ne": "सक्रिय भाषा",
            "hi": "सक्रिय भाषा"
        ],
        "settings_lang_note": [
            "en": "Language is managed natively by iOS. To switch between English, Nepali, and Hindi, open System Settings.",
            "ne": "भाषा iOS द्वारा स्वाभाविक रूपमा व्यवस्थापन गरिन्छ। अंग्रेजी, नेपाली र हिन्दी बीच परिवर्तन गर्न, प्रणाली सेटिङहरू खोल्नुहोस्।",
            "hi": "भाषा का प्रबंधन स्वाभाविक रूप से iOS द्वारा किया जाता है। अंग्रेजी, नेपाली और हिंदी के बीच स्विच करने के लिए, सिस्टम सेटिंग्स खोलें।"
        ],
        "settings_open_button": [
            "en": "Open System Settings",
            "ne": "प्रणाली सेटिङ खोल्नुहोस्",
            "hi": "सिस्टम सेटिंग्स खोलें"
        ],
        "settings_sensors_section": [
            "en": "Sensory & Hardware Diagnostics",
            "ne": "सेन्सर र हार्डवेयर निदान",
            "hi": "सेंसरी और हार्डवेयर डायग्नोस्टिक्स"
        ],
        "settings_truedepth_title": [
            "en": "TrueDepth Infrared Array",
            "ne": "TrueDepth इन्फ्रारेड एरे",
            "hi": "TrueDepth इन्फ्रारेड ऐरे"
        ],
        "settings_truedepth_available": [
            "en": "Available (ARKit 6 Face Tracking)",
            "ne": "उपलब्ध (ARKit अनुहार ट्र्याकिङ)",
            "hi": "उपलब्ध (ARKit चेहरा ट्रैकिंग)"
        ],
        "settings_simulation_mode": [
            "en": "Simulation Mode Active",
            "ne": "सिमुलेशन मोड सक्रिय",
            "hi": "सिमुलेशन मोड सक्रिय"
        ],
        "settings_screen_raycaster": [
            "en": "Screen Raycaster Calibration",
            "ne": "स्क्रिन रेकास्टर क्यालिब्रेसन",
            "hi": "स्क्रीन रेकास्टर कैलिब्रेशन"
        ],
        "settings_screen_raycaster_sub": [
            "en": "Physical Millimeter to Point Mapper",
            "ne": "भौतिक मिलिमिटर देखि पोइन्ट म्यापर",
            "hi": "भौतिक मिलीमीटर से पॉइंट मैपर"
        ],
        "settings_coremotion": [
            "en": "CoreMotion Inertial Filter",
            "ne": "CoreMotion इनर्सियल फिल्टर",
            "hi": "CoreMotion इनर्शियल फ़िल्टर"
        ],
        "settings_coremotion_sub": [
            "en": "Caregiver Tilt & Shake Filter",
            "ne": "टिल्ट र कम्पन फिल्टर",
            "hi": "टिल्ट और शेक फ़िल्टर"
        ],
        "settings_privacy_section": [
            "en": "Privacy & Research Compliance",
            "ne": "गोपनीयता र अनुसन्धान अनुपालन",
            "hi": "गोपनीयता और अनुसंधान अनुपालन"
        ],
        "settings_zero_video_title": [
            "en": "Zero Raw Video Recording",
            "ne": "कुनै भिडियो रेकर्डिङ छैन",
            "hi": "शून्य रॉ वीडियो रिकॉर्डिंग"
        ],
        "settings_zero_video_desc": [
            "en": "Camera frames are processed strictly in volatile RAM. No images or videos are ever written to disk storage.",
            "ne": "क्यामेरा फ्रेमहरू केवल अस्थायी र्‍याममा प्रशोधन हुन्छन्। कुनै पनि तस्बिर वा भिडियोहरू डिस्कमा सुरक्षित गरिँदैन।",
            "hi": "कैमरा फ्रेम केवल वोलाटाइल रैम में प्रोसेस होते हैं। कोई भी इमेज या वीडियो डिस्क पर सेव नहीं की जाती है।"
        ],
        "settings_encrypted_vault_title": [
            "en": "Encrypted Local Vault",
            "ne": "इन्क्रिप्टेड स्थानीय भल्ट",
            "hi": "एन्क्रिप्टेड लोकल वॉल्ट"
        ],
        "settings_encrypted_vault_desc": [
            "en": "Numerical gaze time-series are encrypted at rest using Apple CryptoKit (AES-GCM-256).",
            "ne": "संख्यात्मक दृष्टि डेटा Apple CryptoKit (AES-GCM-256) प्रयोग गरी सुरक्षित गरिन्छ।",
            "hi": "न्यूमेरिकल दृष्टि डेटा Apple CryptoKit (AES-GCM-256) का उपयोग करके एन्क्रिप्ट किया जाता है।"
        ],
        "settings_about_section": [
            "en": "About",
            "ne": "बारेमा",
            "hi": "के बारे में"
        ],
        "settings_app_version": [
            "en": "Application Version",
            "ne": "अनुप्रयोग संस्करण",
            "hi": "एप्लिकेशन संस्करण"
        ],
        "settings_regulatory_class": [
            "en": "Regulatory Classification",
            "ne": "नियामक वर्गीकरण",
            "hi": "नियामक वर्गीकरण"
        ],
        "settings_regulatory_value": [
            "en": "Investigational Device Exemption (IDE)",
            "ne": "अनुसन्धान उपकरण छुट (IDE)",
            "hi": "अनुसंधान उपकरण छूट (IDE)"
        ],
        "settings_dev_lab": [
            "en": "Developer & Clinical Lab",
            "ne": "विकासकर्ता र क्लिनिकल प्रयोगशाला",
            "hi": "डेवलपर और क्लिनिकल लैब"
        ],
        "settings_status_active": [
            "en": "Active",
            "ne": "सक्रिय",
            "hi": "सक्रिय"
        ],
        "settings_status_simulation": [
            "en": "Simulation",
            "ne": "सिमुलेशन",
            "hi": "सिमुलेशन"
        ],
        "settings_calibrated": [
            "en": "Calibrated",
            "ne": "क्यालिब्रेट भयो",
            "hi": "कैलिब्रेटेड"
        ],
        "settings_uncalibrated": [
            "en": "Uncalibrated",
            "ne": "क्यालिब्रेट नभएको",
            "hi": "अनकैलिब्रेटेड"
        ],
        "settings_calibrated_sub": [
            "en": "Per-user correction active",
            "ne": "प्रति-प्रयोगकर्ता सुधार सक्रिय",
            "hi": "प्रति-उपयोगकर्ता सुधार सक्रिय"
        ],
        "settings_uncalibrated_sub": [
            "en": "Raw TrueDepth estimation active",
            "ne": "सामान्य TrueDepth अनुमान सक्रिय",
            "hi": "सामान्य TrueDepth अनुमान सक्रिय"
        ],
        "settings_reset_calibration": [
            "en": "Reset Eye-Tracking Calibration",
            "ne": "दृष्टि क्यालिब्रेसन रिसेट गर्नुहोस्",
            "hi": "दृष्टि कैलिब्रेशन रीसेट करें"
        ]
    ]

    /// Translates key using device's preferred locale from iOS Settings.
    public static func tr(_ key: String, lang: AppLocale? = nil) -> String {
        let activeLocale = lang ?? AppLocale.current
        return strings[key]?[activeLocale.rawValue] ?? strings[key]?["en"] ?? key
    }
}
