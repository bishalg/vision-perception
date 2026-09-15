import SwiftUI

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
            "en": "Dashboard",
            "ne": "ड्यासबोर्ड",
            "hi": "डैशबोर्ड"
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
            "en": "Gaze",
            "ne": "दृष्टि",
            "hi": "दृष्टि"
        ],
        "tab_settings": [
            "en": "Settings",
            "ne": "सेटिङहरू",
            "hi": "सेटिंग्स"
        ],
        "wizard_step1_title": [
            "en": "Position Camera",
            "ne": "क्यामेरा मिलाउनुहोस्",
            "hi": "कैमरा सेट करें"
        ],
        "wizard_step1_subtitle": [
            "en": "Hold phone at the child's eye level",
            "ne": "फोनलाई बच्चाको आँखाको तहमा राख्नुहोस्",
            "hi": "फ़ोन को बच्चे की आँख के स्तर पर रखें"
        ],
        "wizard_step2_title": [
            "en": "Calibration",
            "ne": "क्यालिब्रेसन",
            "hi": "कैलिब्रेशन"
        ],
        "wizard_step3_title": [
            "en": "Screening",
            "ne": "स्क्रिनिङ",
            "hi": "स्क्रीनिंग"
        ],
        "wizard_begin_screening": [
            "en": "Begin Screening",
            "ne": "स्क्रिनिङ सुरु गर्नुहोस्",
            "hi": "स्क्रीनिंग शुरू करें"
        ],
        "wizard_ready_calibrate": [
            "en": "Ready – Start Calibration",
            "ne": "तयार छ – क्यालिब्रेसन सुरु गर्नुहोस्",
            "hi": "तैयार – कैलिब्रेशन शुरू करें"
        ],
        "wizard_skip_calibrate": [
            "en": "Skip (Use Saved Calibration)",
            "ne": "छोड्नुहोस् (सुरक्षित क्यालिब्रेसन प्रयोग गर्नुहोस्)",
            "hi": "छोड़ें (सहेजी गई कैलिब्रेशन उपयोग करें)"
        ],
        "wizard_distance_good": [
            "en": "Perfect distance · Keep steady",
            "ne": "उत्तम दूरी · स्थिर राख्नुहोस्",
            "hi": "सही दूरी · स्थिर रखें"
        ],
        "wizard_distance_close": [
            "en": "Too close · Move phone back",
            "ne": "धेरै नजिक · फोन पछाडि सार्नुहोस्",
            "hi": "बहुत पास · फ़ोन पीछे करें"
        ],
        "wizard_distance_far": [
            "en": "Too far · Bring phone closer",
            "ne": "धेरै टाढा · फोन नजिक ल्याउनुहोस्",
            "hi": "बहुत दूर · फ़ोन पास लाएं"
        ],
        "wizard_no_face": [
            "en": "Point camera at child's face",
            "ne": "क्यामेरा बच्चाको अनुहारतिर देखाउनुहोस्",
            "hi": "कैमरा बच्चे के चेहरे की ओर करें"
        ],
        "wizard_hold_steady": [
            "en": "Hold steady for",
            "ne": "स्थिर राख्नुहोस्",
            "hi": "स्थिर रखें"
        ],
        "settings_screening_tools_section": [
            "en": "Screening & Calibration Tools",
            "ne": "स्क्रिनिङ र क्यालिब्रेसन उपकरणहरू",
            "hi": "स्क्रीनिंग और कैलिब्रेशन उपकरण"
        ],
        "settings_quick_calibrate": [
            "en": "Quick Calibrate",
            "ne": "द्रुत क्यालिब्रेसन",
            "hi": "त्वरित कैलिब्रेशन"
        ],
        "settings_quick_calibrate_sub": [
            "en": "Run the 5-point bubble calibration",
            "ne": "५-पोइन्ट बबल क्यालिब्रेसन चलाउनुहोस्",
            "hi": "5-पॉइंट बबल कैलिब्रेशन चलाएं"
        ],
        "settings_camera_check": [
            "en": "Camera & Distance Check",
            "ne": "क्यामेरा र दूरी जाँच",
            "hi": "कैमरा और दूरी जाँच"
        ],
        "settings_camera_check_sub": [
            "en": "Verify face positioning before screening",
            "ne": "स्क्रिनिङ अघि अनुहारको स्थिति जाँच गर्नुहोस्",
            "hi": "स्क्रीनिंग से पहले चेहरे की स्थिति जाँचें"
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
        "carekit_meds_title": [
            "en": "Daily Medication Schedule",
            "ne": "दैनिक औषधि तालिका",
            "hi": "दैनिक दवा समय-सारणी"
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
        ],
        "meds_add": [
            "en": "Add medicine",
            "ne": "औषधि थप्नुहोस्",
            "hi": "दवा जोड़ें"
        ],
        "meds_empty": [
            "en": "Add a medicine or supplement and set reminder times.",
            "ne": "औषधि वा सप्लिमेन्ट थप्नुहोस् र रिमाइन्डर समय सेट गर्नुहोस्।",
            "hi": "दवा या सप्लिमेंट जोड़ें और रिमाइंडर समय सेट करें।"
        ],
        "meds_edit_schedule": [
            "en": "Edit schedule & reminders",
            "ne": "तालिका र रिमाइन्डर सम्पादन गर्नुहोस्",
            "hi": "शेड्यूल और रिमाइंडर संपादित करें"
        ],
        "diary_history_reminders": [
            "en": "History & reminders",
            "ne": "इतिहास र रिमाइन्डर",
            "hi": "इतिहास और रिमाइंडर"
        ],
        "settings_alerts_section": [
            "en": "Notifications & alerts",
            "ne": "सूचना र अलर्ट",
            "hi": "सूचनाएं और अलर्ट"
        ],
        "settings_alerts_master": [
            "en": "Enable reminders and alerts",
            "ne": "रिमाइन्डर र अलर्ट सक्षम गर्नुहोस्",
            "hi": "रिमाइंडर और अलर्ट चालू करें"
        ],
        "settings_alerts_alarmkit": [
            "en": "Use AlarmKit when available",
            "ne": "उपलब्ध भए AlarmKit प्रयोग गर्नुहोस्",
            "hi": "उपलब्ध होने पर AlarmKit उपयोग करें"
        ],
        "settings_alerts_island": [
            "en": "Dynamic Island countdown (opt-in)",
            "ne": "Dynamic Island काउन्टडाउन (ऐच्छिक)",
            "hi": "Dynamic Island काउंटडाउन (वैकल्पिक)"
        ],
        "settings_alerts_status": [
            "en": "Notification permission",
            "ne": "सूचना अनुमति",
            "hi": "सूचना अनुमति"
        ],
        "settings_alerts_open": [
            "en": "Open iOS Settings",
            "ne": "iOS सेटिङ खोल्नुहोस्",
            "hi": "iOS सेटिंग खोलें"
        ],
        "settings_conditions_section": [
            "en": "Medical conditions",
            "ne": "चिकित्सा अवस्थाहरू",
            "hi": "चिकित्सा स्थितियाँ"
        ]
    ]

    /// Translates key using device's preferred locale from iOS Settings.
    public static func tr(_ key: String, lang: AppLocale? = nil) -> String {
        let activeLocale = lang ?? AppLocale.current
        return strings[key]?[activeLocale.rawValue] ?? strings[key]?["en"] ?? key
    }
}
