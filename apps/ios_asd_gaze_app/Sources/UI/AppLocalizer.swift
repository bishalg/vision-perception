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
            "en": "Visual attention observations & milestones",
            "ne": "दृष्टि ध्यान अवलोकन र विकास चरण",
            "hi": "दृष्टि ध्यान अवलोकन और विकास चरण"
        ],
        "start_screening": [
            "en": "Start Observation Session",
            "ne": "अवलोकन सत्र सुरु गर्नुहोस्",
            "hi": "अवलोकन सत्र शुरू करें"
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
        "social_attention_title": [
            "en": "Social Attention",
            "ne": "सामाजिक ध्यान",
            "hi": "सामाजिक ध्यान"
        ],
        "emotion_title": [
            "en": "Facial Expression",
            "ne": "अनुहारको भाव",
            "hi": "चेहरे के भाव"
        ],
        "export_researchkit": [
            "en": "Export Data File (JSON)",
            "ne": "डाटा फाइल (JSON) निर्यात गर्नुहोस्",
            "hi": "डेटा फ़ाइल (JSON) निर्यात करें"
        ],
        "export_pdf": [
            "en": "Export PDF Report",
            "ne": "PDF प्रतिवेदन निर्यात गर्नुहोस्",
            "hi": "PDF रिपोर्ट निर्यात करें"
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
            "en": "Observation",
            "ne": "अवलोकन",
            "hi": "अवलोकन"
        ],
        "wizard_begin_screening": [
            "en": "Begin Observation",
            "ne": "अवलोकन सुरु गर्नुहोस्",
            "hi": "अवलोकन शुरू करें"
        ],
        "wizard_ready_calibrate": [
            "en": "Ready – Start Calibration",
            "ne": "तयार छ – क्यालिब्रेसन सुरु गर्नुहोस्",
            "hi": "तैयार – कैलिब्रेशन शुरू करें"
        ],
        "calibrate_failed_title": [
            "en": "Calibration did not finish",
            "ne": "क्यालिब्रेसन पूरा भएन",
            "hi": "कैलिब्रेशन पूरा नहीं हुआ"
        ],
        "calibrate_failed_body": [
            "en": "We could not see your child's eyes for one of the balloons. Check the lighting and distance, then try again.",
            "ne": "एउटा बेलुनको लागि हामीले तपाईंको बच्चाको आँखा देख्न सकेनौं। उज्यालो र दूरी जाँच गर्नुहोस्, अनि फेरि प्रयास गर्नुहोस्।",
            "hi": "एक गुब्बारे के लिए हम आपके बच्चे की आँखें नहीं देख सके। रोशनी और दूरी जाँचें, फिर दोबारा कोशिश करें।"
        ],
        "calibrate_try_again": [
            "en": "Try again",
            "ne": "फेरि प्रयास गर्नुहोस्",
            "hi": "फिर कोशिश करें"
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
            "en": "Observation & Calibration Tools",
            "ne": "अवलोकन र क्यालिब्रेसन उपकरणहरू",
            "hi": "अवलोकन और कैलिब्रेशन उपकरण"
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
            "en": "Verify face positioning before observing",
            "ne": "अवलोकन अघि अनुहारको स्थिति जाँच गर्नुहोस्",
            "hi": "अवलोकन से पहले चेहरे की स्थिति जाँचें"
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
            "en": "Export CareKit Summary",
            "ne": "CareKit सारांश निर्यात गर्नुहोस्",
            "hi": "CareKit सारांश निर्यात करें"
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
            "en": "IMPORTANT NOTE",
            "ne": "महत्वपूर्ण जानकारी",
            "hi": "महत्वपूर्ण सूचना"
        ],
        "dashboard_clinical_title": [
            "en": "Where Your Child Looks",
            "ne": "तपाईंको बच्चा कहाँ हेर्छ",
            "hi": "आपका बच्चा कहाँ देखता है"
        ],
        "dashboard_clinical_desc": [
            "en": "Measures where your child looks during short clips. This is not a diagnostic tool. Please talk with your pediatrician about any concerns.",
            "ne": "छोटा क्लिपहरूमा तपाईंको बच्चाले कहाँ हेर्छ भन्ने मापन गर्दछ। यो निदान गर्ने उपकरण होइन। कुनै चिन्ता भए बाल रोग विशेषज्ञसँग कुरा गर्नुहोस्।",
            "hi": "छोटे क्लिप के दौरान आपका बच्चा कहाँ देखता है, यह मापता है। यह निदान करने का उपकरण नहीं है। कोई चिंता हो तो बाल रोग विशेषज्ञ से बात करें।"
        ],
        "dashboard_more_details": [
            "en": "More Details",
            "ne": "थप विवरण",
            "hi": "और विवरण"
        ],
        "dashboard_care_activity": [
            "en": "Care Routine Activity",
            "ne": "हेरचाह दिनचर्या गतिविधि",
            "hi": "देखभाल दिनचर्या गतिविधि"
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
        // hi/ne native review pending
        "dashboard_highlight_note": [
            "en": "Sessions recorded this week are saved on this device and ready for your clinic to review.",
            "ne": "यस हप्ता रेकर्ड गरिएका सत्रहरू यस उपकरणमा सुरक्षित छन् र तपाईंको क्लिनिकद्वारा समीक्षाका लागि तयार छन्।",
            "hi": "इस सप्ताह रिकॉर्ड किए गए सत्र इस डिवाइस पर सहेजे गए हैं और आपके क्लिनिक की समीक्षा के लिए तैयार हैं।"
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
            "en": "Sensor & Hardware Checks",
            "ne": "सेन्सर र हार्डवेयर जाँच",
            "hi": "सेंसर और हार्डवेयर जाँच"
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
            "en": "Privacy & Data",
            "ne": "गोपनीयता र डाटा",
            "hi": "गोपनीयता और डेटा"
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
            "en": "App Category",
            "ne": "एप वर्ग",
            "hi": "ऐप श्रेणी"
        ],
        "settings_regulatory_value": [
            "en": "Observation Diary, not a medical device",
            "ne": "अवलोकन डायरी, चिकित्सकीय उपकरण होइन",
            "hi": "अवलोकन डायरी, चिकित्सा उपकरण नहीं"
        ],
        "settings_dev_lab": [
            "en": "Developer Tools",
            "ne": "विकासकर्ता उपकरण",
            "hi": "डेवलपर उपकरण"
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
        "target_calibration_hint": [
            "en": "Watch the balloon!",
            "ne": "बेलुन हेर्नुहोस्!",
            "hi": "गुब्बारे को देखो!"
        ],
        "target_validation_hint": [
            "en": "Find the star!",
            "ne": "तारा खोज्नुहोस्!",
            "hi": "तारा ढूँढो!"
        ],
        "target_check_hint": [
            "en": "Spot the ladybug!",
            "ne": "सानो कीरा खोज्नुहोस्!",
            "hi": "लेडीबग ढूँढो!"
        ],
        "calibrate_result_great": [
            "en": "Great job!",
            "ne": "धेरै राम्रो!",
            "hi": "बहुत बढ़िया!"
        ],
        "calibrate_result_poor_title": [
            "en": "Calibration was not steady",
            "ne": "क्यालिब्रेसन स्थिर भएन",
            "hi": "कैलिब्रेशन स्थिर नहीं रहा"
        ],
        "calibrate_result_poor_body": [
            "en": "The gaze points were spread out. Check the lighting and distance and keep the phone still, then try again, or continue with this calibration.",
            "ne": "दृष्टि बिन्दुहरू छरिएका थिए। उज्यालो र दूरी जाँच गर्नुहोस् र फोन स्थिर राख्नुहोस्, अनि फेरि प्रयास गर्नुहोस्, वा यही क्यालिब्रेसनसँग अगाडि बढ्नुहोस्।",
            "hi": "नज़र के बिंदु बिखरे हुए थे। रोशनी और दूरी जाँचें और फ़ोन स्थिर रखें, फिर दोबारा कोशिश करें, या इसी कैलिब्रेशन के साथ आगे बढ़ें।"
        ],
        "calibrate_continue_anyway": [
            "en": "Continue anyway",
            "ne": "जे भए पनि अगाडि बढ्नुहोस्",
            "hi": "फिर भी आगे बढ़ें"
        ],
        "calibrate_limit_reached": [
            "en": "No more calibration attempts are available in this session.",
            "ne": "यो सत्रमा थप क्यालिब्रेसन प्रयास उपलब्ध छैन।",
            "hi": "इस सत्र में और कैलिब्रेशन प्रयास उपलब्ध नहीं हैं।"
        ],
        "validation_result_recal_title": [
            "en": "The gaze check was off",
            "ne": "दृष्टि जाँच मिलेन",
            "hi": "नज़र की जाँच सही नहीं रही"
        ],
        "validation_result_warn_title": [
            "en": "The gaze check is borderline",
            "ne": "दृष्टि जाँच सीमामा छ",
            "hi": "नज़र की जाँच सीमा पर है"
        ],
        "validation_result_offset": [
            "en": "Measured offset: %.1f°",
            "ne": "मापिएको विचलन: %.1f°",
            "hi": "मापा गया विचलन: %.1f°"
        ],
        "validation_result_missed": [
            "en": "The child did not look at enough of the stars.",
            "ne": "बच्चाले पर्याप्त ताराहरू हेरेन।",
            "hi": "बच्चे ने पर्याप्त तारे नहीं देखे।"
        ],
        "validation_calibrate_again": [
            "en": "Calibrate again",
            "ne": "फेरि क्यालिब्रेट गर्नुहोस्",
            "hi": "फिर से कैलिब्रेट करें"
        ],
        "validation_continue": [
            "en": "Continue",
            "ne": "अगाडि बढ्नुहोस्",
            "hi": "आगे बढ़ें"
        ],
        "recal_prompt_title": [
            "en": "Calibrate again before the next video?",
            "ne": "अर्को भिडियो अघि फेरि क्यालिब्रेट गर्ने?",
            "hi": "अगले वीडियो से पहले फिर से कैलिब्रेट करें?"
        ],
        "recal_prompt_body": [
            "en": "Tracking changed during the last video. A short calibration keeps the gaze measurements usable.",
            "ne": "पछिल्लो भिडियोमा ट्र्याकिङ बदलियो। छोटो क्यालिब्रेसनले दृष्टि मापनलाई प्रयोगयोग्य राख्छ।",
            "hi": "पिछले वीडियो के दौरान ट्रैकिंग बदल गई। एक छोटा कैलिब्रेशन नज़र के माप को उपयोगी बनाए रखता है।"
        ],
        "recal_prompt_accept": [
            "en": "Calibrate now",
            "ne": "अहिले क्यालिब्रेट गर्नुहोस्",
            "hi": "अभी कैलिब्रेट करें"
        ],
        "recal_prompt_decline": [
            "en": "Keep going",
            "ne": "जारी राख्नुहोस्",
            "hi": "जारी रखें"
        ],
        "recal_reason_quality": [
            "en": "The calibration quality was low.",
            "ne": "क्यालिब्रेसनको गुणस्तर कम थियो।",
            "hi": "कैलिब्रेशन की गुणवत्ता कम थी।"
        ],
        "recal_reason_validation": [
            "en": "The gaze check after calibration was off.",
            "ne": "क्यालिब्रेसनपछिको दृष्टि जाँच मिलेन।",
            "hi": "कैलिब्रेशन के बाद नज़र की जाँच सही नहीं रही।"
        ],
        "recal_reason_drift": [
            "en": "The gaze has drifted since calibration.",
            "ne": "क्यालिब्रेसनपछि दृष्टि सरेको छ।",
            "hi": "कैलिब्रेशन के बाद से नज़र खिसक गई है।"
        ],
        "recal_reason_pose": [
            "en": "The child's head moved away from the calibration position.",
            "ne": "बच्चाको टाउको क्यालिब्रेसनको स्थितिबाट सरेको छ।",
            "hi": "बच्चे का सिर कैलिब्रेशन की स्थिति से हट गया है।"
        ],
        "recal_reason_orientation": [
            "en": "The device was turned.",
            "ne": "उपकरण घुमाइयो।",
            "hi": "डिवाइस घुमाया गया।"
        ],
        "recal_reason_age": [
            "en": "The calibration is getting old.",
            "ne": "क्यालिब्रेसन पुरानो हुँदैछ।",
            "hi": "कैलिब्रेशन पुराना हो रहा है।"
        ],
        "recal_reason_operator": [
            "en": "Requested by the operator.",
            "ne": "सञ्चालकको अनुरोधमा।",
            "hi": "संचालक के अनुरोध पर।"
        ],
        "settings_conditions_section": [
            "en": "Medical conditions",
            "ne": "चिकित्सा अवस्थाहरू",
            "hi": "चिकित्सा स्थितियाँ"
        ],
        // SYNC-8 clinic upload status line (counts only).
        "sync_status_waiting": [
            "en": "%d waiting",
            "ne": "%d पर्खाइमा",
            "hi": "%d प्रतीक्षा में"
        ],
        "sync_status_uploading": [
            "en": "%d uploading",
            "ne": "%d अपलोड हुँदै",
            "hi": "%d अपलोड हो रहे"
        ],
        "sync_status_attention": [
            "en": "%d need attention",
            "ne": "%d लाई ध्यान चाहिन्छ",
            "hi": "%d पर ध्यान चाहिए"
        ],
        "sync_status_last_synced": [
            "en": "last synced %@",
            "ne": "पछिल्लो सिङ्क %@",
            "hi": "अंतिम सिंक %@"
        ],
        "sync_status_never": [
            "en": "not synced yet",
            "ne": "अझै सिङ्क भएको छैन",
            "hi": "अभी सिंक नहीं हुआ"
        ],
        "sync_status_retry": [
            "en": "Retry",
            "ne": "फेरि प्रयास",
            "hi": "फिर से प्रयास"
        ],
        "research_raw_samples_note": [
            "en": "Raw gaze samples are shared for research when you opt in",
            "ne": "तपाईंले छनोट गर्नुभएमा अनुसन्धानका लागि कच्चा नजर नमूनाहरू साझा गरिन्छ",
            "hi": "आपके चुनने पर अनुसंधान के लिए कच्चे गेज नमूने साझा किए जाते हैं"
        ],
        // 2026-11-08 store build; hi/ne native review pending
        "store_tab_capture": [
            "en": "Capture",
            "ne": "क्याप्चर",
            "hi": "कैप्चर"
        ],
        "store_tab_sessions": [
            "en": "Sessions",
            "ne": "सत्रहरू",
            "hi": "सत्र"
        ],
        "mode_select_prompt": [
            "en": "How will you use EarlyGaze?",
            "ne": "तपाईं EarlyGaze कसरी प्रयोग गर्नुहुन्छ?",
            "hi": "आप EarlyGaze का उपयोग कैसे करेंगे?"
        ],
        "about_records_gaze": [
            "en": "It records where a child looks during short clips.",
            "ne": "यसले छोटा क्लिपहरूमा बच्चाले कहाँ हेर्छ भनेर रेकर्ड गर्छ।",
            "hi": "यह छोटे क्लिप के दौरान बच्चा कहाँ देखता है, इसे रिकॉर्ड करता है।"
        ],
        "recording_quality": [
            "en": "Recording quality",
            "ne": "रेकर्डिङको गुणस्तर",
            "hi": "रिकॉर्डिंग की गुणवत्ता"
        ],
        "recording_quality_format": [
            "en": "Usable samples: %.1f%% • Quality: %@",
            "ne": "प्रयोगयोग्य नमूना: %.1f%% • गुणस्तर: %@",
            "hi": "उपयोग योग्य नमूने: %.1f%% • गुणवत्ता: %@"
        ],
        "saved_on_device": [
            "en": "Saved on this device",
            "ne": "यो उपकरणमा सुरक्षित गरियो",
            "hi": "इस डिवाइस पर सहेजा गया"
        ],
        "milestone_background": [
            "en": "Background",
            "ne": "पृष्ठभूमि",
            "hi": "पृष्ठभूमि"
        ],
        "onb_age_bracket": [
            "en": "Age Bracket",
            "ne": "उमेर समूह",
            "hi": "आयु वर्ग"
        ],
        "onb_age_group_prefix": [
            "en": "Age group:",
            "ne": "उमेर समूह:",
            "hi": "आयु समूह:"
        ],
        "onb_notes_prefix": [
            "en": "Notes:",
            "ne": "टिप्पणी:",
            "hi": "नोट्स:"
        ],
        "settings_gaze_capture_sub": [
            "en": "Gaze observation capture",
            "ne": "नजर अवलोकन क्याप्चर",
            "hi": "गेज अवलोकन कैप्चर"
        ],
        // Store build Settings, sensor rows in plain words (S-6).
        "store_settings_gaze_calibration": [
            "en": "Gaze calibration",
            "ne": "दृष्टि क्यालिब्रेसन",
            "hi": "दृष्टि कैलिब्रेशन"
        ],
        "store_face_tracking_available": [
            "en": "Face tracking available",
            "ne": "अनुहार ट्र्याकिङ उपलब्ध",
            "hi": "चेहरा ट्रैकिंग उपलब्ध"
        ],
        "store_face_tracking_unavailable": [
            "en": "Face tracking not available on this device",
            "ne": "यो उपकरणमा अनुहार ट्र्याकिङ उपलब्ध छैन",
            "hi": "इस डिवाइस पर चेहरा ट्रैकिंग उपलब्ध नहीं है"
        ],
        // Post-session title in the store build (S-3): no "report" on the device.
        "store_result_title": [
            "en": "Observation saved",
            "ne": "अवलोकन सुरक्षित भयो",
            "hi": "अवलोकन सहेजा गया"
        ],
        "store_no_sessions": [
            "en": "No observation sessions yet.",
            "ne": "अहिलेसम्म कुनै अवलोकन सत्र छैन।",
            "hi": "अभी तक कोई अवलोकन सत्र नहीं है।"
        ]
    ]

    /// Translates key using device's preferred locale from iOS Settings.
    public static func tr(_ key: String, lang: AppLocale? = nil) -> String {
        let activeLocale = lang ?? AppLocale.current
        return strings[key]?[activeLocale.rawValue] ?? strings[key]?["en"] ?? key
    }
}
