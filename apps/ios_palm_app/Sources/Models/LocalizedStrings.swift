import Foundation

/**
 * Internationalization (i18n) engine for Palm Perception mobile clients.
 * Supports English (en), Nepali (ne), and Hindi (hi).
 */
public enum AppLanguage: String, CaseIterable, Identifiable {
    case english = "en"
    case nepali = "ne"
    case hindi = "hi"

    public var id: String { rawValue }

    public var displayName: String {
        switch self {
        case .english: return "English"
        case .nepali: return "नेपाली"
        case .hindi: return "हिन्दी"
        }
    }
}

public struct LocalizedStrings {
    public static var currentLanguage: AppLanguage = .english

    private static let table: [String: [AppLanguage: String]] = [
        "guidance.searching": [
            .english: "Place your palm inside the frame",
            .nepali: "हात फ्रेम भित्र राख्नुहोस्",
            .hindi: "हथेली को फ्रेम के अंदर रखें"
        ],
        "guidance.center": [
            .english: "Center your palm",
            .nepali: "हातलाई केन्द्रमा राख्नुहोस्",
            .hindi: "हथेली को केंद्र में रखें"
        ],
        "guidance.closer": [
            .english: "Move closer",
            .nepali: "हात नजिक ल्याउनुहोस्",
            .hindi: "हथेली पास लाएं"
        ],
        "guidance.further": [
            .english: "Move further away",
            .nepali: "हात अलि टाढा लैजानुहोस्",
            .hindi: "हथेली थोड़ा दूर ले जाएं"
        ],
        "guidance.flat": [
            .english: "Hold palm flat facing camera",
            .nepali: "हात सीधा क्यामेरा तर्फ देखाउनुहोस्",
            .hindi: "हथेली को कैमरे के सामने सीधा रखें"
        ],
        "guidance.dark": [
            .english: "Too dark — add more light",
            .nepali: "धेरै अँध्यारो छ — थप उज्यालो चाहिन्छ",
            .hindi: "बहुत अंधेरा है — रोशनी बढ़ाएं"
        ],
        "guidance.glare": [
            .english: "Angle palm away from glare",
            .nepali: "सिधा प्रकाशबाट हात अलि ढल्काउनुहोस्",
            .hindi: "सीधी रोशनी से हथेली थोड़ा घुमाएं"
        ],
        "guidance.steady": [
            .english: "Hold steady...",
            .nepali: "हात स्थिर राख्नुहोस्...",
            .hindi: "स्थिर रखें..."
        ],
        "guidance.ready": [
            .english: "Capturing palm lines...",
            .nepali: "हातका रेखाहरू खिचिँदैछ...",
            .hindi: "हथेली की रेखाएं कैप्चर हो रही हैं..."
        ],
        "line.heart": [
            .english: "Heart Line",
            .nepali: "हृदय रेखा",
            .hindi: "हृदय रेखा"
        ],
        "line.head": [
            .english: "Head Line",
            .nepali: "मस्तिष्क रेखा",
            .hindi: "मस्तिष्क रेखा"
        ],
        "line.life": [
            .english: "Life Line",
            .nepali: "जीवन रेखा",
            .hindi: "जीवन रेखा"
        ],
        "line.fate": [
            .english: "Fate Line",
            .nepali: "भाग्य रेखा",
            .hindi: "भाग्य रेखा"
        ],
        "button.capture": [
            .english: "Capture",
            .nepali: "खिच्नुहोस्",
            .hindi: "कैप्चर"
        ],
        "button.retake": [
            .english: "Retake",
            .nepali: "पुनः खिच्नुहोस्",
            .hindi: "फिर से लें"
        ],
        "sheet.title": [
            .english: "Samudrika Palm Analysis",
            .nepali: "सामुद्रिक हस्तरेखा विश्लेषण",
            .hindi: "सामुद्रिक हस्तरेखा विश्लेषण"
        ],
        "status.processing": [
            .english: "Mapping palm lines...",
            .nepali: "हातका रेखाहरू म्याप हुँदैछ...",
            .hindi: "हथेली की रेखाएं मैप हो रही हैं..."
        ],
        "result.title": [
            .english: "Palm Line Overlay",
            .nepali: "हस्तरेखा ओभरले",
            .hindi: "हस्तरेखा ओवरले"
        ],
        "result.landmark": [
            .english: "Landmark guides (native engine not linked)",
            .nepali: "ल्यान्डमार्क गाइड (नेटिभ इन्जिन लिंक छैन)",
            .hindi: "लैंडमार्क गाइड (नेटिव इंजन लिंक नहीं)"
        ],
        "result.native": [
            .english: "Native CurvEngine crease polylines",
            .nepali: "नेटिभ CurvEngine रेखा पोलिलाइन",
            .hindi: "नेटिव CurvEngine रेखा पॉलीलाइन"
        ],
        "result.retake": [
            .english: "Retake",
            .nepali: "फेरि खिच्नुहोस्",
            .hindi: "फिर से लें"
        ],
        "result.edit": [
            .english: "Edit",
            .nepali: "सम्पादन",
            .hindi: "संपादन"
        ],
        "result.doneEdit": [
            .english: "Done",
            .nepali: "सकियो",
            .hindi: "हो गया"
        ],
        "result.editHint": [
            .english: "Drag handles to reshape · drag rows to reorder · tag to reassign Heart/Head/Life/Fate",
            .nepali: "ह्यान्डल तानेर मिलाउनुहोस् · पङ्क्ति सार्नुहोस् · ट्यागबाट नाम बदल्नुहोस्",
            .hindi: "हैंडल खींचकर सुधारें · पंक्तियों को क्रम दें · टैग से नाम बदलें"
        ],
        "result.adjusted": [
            .english: "adjusted",
            .nepali: "मिलान",
            .hindi: "समायोजित"
        ],
        "result.useAdjusted": [
            .english: "Use these lines for reading",
            .nepali: "यी रेखाहरू पढाइका लागि प्रयोग गर्नुहोस्",
            .hindi: "पढ़ने के लिए इन रेखाओं का उपयोग करें"
        ],
        "result.continueReading": [
            .english: "Analyze Reading",
            .nepali: "सामुद्रिक विश्लेषण हेर्नुहोस्",
            .hindi: "सामुद्रिक विश्लेषण देखें"
        ],
        "editor.title": [
            .english: "Edit Crease Lines",
            .nepali: "हस्तरेखा सम्पादन",
            .hindi: "हस्तरेखा संपादन"
        ],
        "editor.cancel": [
            .english: "Cancel",
            .nepali: "रद्द गर्नुहोस्",
            .hindi: "रद्द करें"
        ],
        "editor.done": [
            .english: "Done",
            .nepali: "सम्पन्न",
            .hindi: "संपन्न"
        ],
        "editor.addPoint": [
            .english: "+ Point",
            .nepali: "+ बिन्दु",
            .hindi: "+ बिंदु"
        ],
        "editor.removePoint": [
            .english: "Delete Point",
            .nepali: "बिन्दु मेटाउनुहोस्",
            .hindi: "बिंदु हटाएं"
        ],
        "editor.resetLine": [
            .english: "Reset Line",
            .nepali: "लाइन रिसेट",
            .hindi: "लाइन रीसेट"
        ],
        "editor.undo": [
            .english: "Undo",
            .nepali: "पूर्वावस्था",
            .hindi: "पूर्ववत"
        ],
        "editor.hint": [
            .english: "Drag handles to reshape. Magnifier shows crease details under finger.",
            .nepali: "ह्यान्डल सारेर मिलाउनुहोस्। म्याग्निफायरले औंला मुनिको रेखा देखाउँछ।",
            .hindi: "हैंडल खींचकर सुधारें। मैग्निफायर उंगली के नीचे की रेखा दिखाता है।"
        ],
        "editor.resetZoom": [
            .english: "1.0x",
            .nepali: "१.०x",
            .hindi: "१.०x"
        ],
        "editor.modePoints": [
            .english: "Adjust",
            .nepali: "मिलाउनुहोस्",
            .hindi: "समायोजित"
        ],
        "editor.modeDraw": [
            .english: "Draw Curve",
            .nepali: "रेखा कोर्नुहोस्",
            .hindi: "रेखा बनाएं"
        ],
        "editor.drawHint": [
            .english: "Trace along the palm crease with your finger to redraw the curve.",
            .nepali: "हातको रेखामाथि औंला सारेर नयाँ वक्र रेखा कोर्नुहोस्।",
            .hindi: "हथेली की रेखा पर उंगली फिराकर नई वक्र रेखा बनाएं।"
        ],
        "editor.toolAdjust": [
            .english: "Adjust",
            .nepali: "सार्नुहोस्",
            .hindi: "समायोजित"
        ],
        "editor.toolDraw": [
            .english: "Draw",
            .nepali: "कोर्नुहोस्",
            .hindi: "खींचें"
        ],
        "editor.toolUndo": [
            .english: "Undo",
            .nepali: "फिर्ता",
            .hindi: "पूर्ववत"
        ],
        "editor.toolReset": [
            .english: "Reset",
            .nepali: "रिसेट",
            .hindi: "रीसेट"
        ],
        "editor.toolLabel": [
            .english: "Label",
            .nepali: "नाम",
            .hindi: "लेबल"
        ],
        "editor.hintAdjust": [
            .english: "Drag handles to reshape • Pinch to zoom",
            .nepali: "ह्यान्डल सारेर मिलाउनुहोस् • जुम गर्न पिन्च गर्नुहोस्",
            .hindi: "हैंडल खींचकर सुधारें • ज़ूम के लिए पिंच करें"
        ],
        "editor.hintDraw": [
            .english: "Trace along palm crease to redraw",
            .nepali: "हस्तरेखामाथि औंला सारेर नयाँ रेखा कोर्नुहोस्",
            .hindi: "हथेली की रेखा पर उंगली फिराकर नई रेखा बनाएं"
        ],
        "editor.hintMovingStart": [
            .english: "Moving Start Point (S)",
            .nepali: "सुरुवात बिन्दु (S) सार्दै",
            .hindi: "शुरुआती बिंदु (S) स्थानांतरित"
        ],
        "editor.hintMovingCurve": [
            .english: "Adjusting curve curvature",
            .nepali: "वक्रता मिलाउँदै",
            .hindi: "वक्रता समायोजित कर रहे हैं"
        ],
        "editor.hintMovingEnd": [
            .english: "Moving End Point (E)",
            .nepali: "अन्त्य बिन्दु (E) सार्दै",
            .hindi: "अंतिम बिंदु (E) स्थानांतरित"
        ],
        "editor.hintTracing": [
            .english: "Tracing curve... release to set line",
            .nepali: "रेखा कोरिरहेको... औंला उठाएर रेखा बनाउनुहोस्",
            .hindi: "रेखा खींच रहे हैं... उंगली उठाकर रेखा बनाएं"
        ]
    ]

    public static func tr(_ key: String, lang: AppLanguage = currentLanguage) -> String {
        return table[key]?[lang] ?? table[key]?[.english] ?? key
    }
}
