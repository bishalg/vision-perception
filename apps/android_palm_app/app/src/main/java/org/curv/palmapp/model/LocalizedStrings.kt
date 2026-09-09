package org.curv.palmapp.model

/**
 * Internationalization (i18n) strings engine for Android Palm Perception app.
 * Supports English (en), Nepali (ne), and Hindi (hi) with 1:1 parity with iOS.
 */
enum class AppLanguage(val code: String, val displayName: String) {
    ENGLISH("en", "English"),
    NEPALI("ne", "नेपाली"),
    HINDI("hi", "हिन्दी")
}

object LocalizedStrings {
    var currentLanguage: AppLanguage = AppLanguage.ENGLISH

    private val table: Map<String, Map<AppLanguage, String>> = mapOf(
        "guidance.searching" to mapOf(
            AppLanguage.ENGLISH to "Place your palm inside the frame",
            AppLanguage.NEPALI to "हात फ्रेम भित्र राख्नुहोस्",
            AppLanguage.HINDI to "हथेली को फ्रेम के अंदर रखें"
        ),
        "guidance.center" to mapOf(
            AppLanguage.ENGLISH to "Center your palm",
            AppLanguage.NEPALI to "हातलाई केन्द्रमा राख्नुहोस्",
            AppLanguage.HINDI to "हथेली को केंद्र में रखें"
        ),
        "guidance.closer" to mapOf(
            AppLanguage.ENGLISH to "Move closer",
            AppLanguage.NEPALI to "हात नजिक ल्याउनुहोस्",
            AppLanguage.HINDI to "हथेली पास लाएं"
        ),
        "guidance.further" to mapOf(
            AppLanguage.ENGLISH to "Move further away",
            AppLanguage.NEPALI to "हात अलि टाढा लैजानुहोस्",
            AppLanguage.HINDI to "हथेली थोड़ा दूर ले जाएं"
        ),
        "guidance.flat" to mapOf(
            AppLanguage.ENGLISH to "Hold palm flat facing camera",
            AppLanguage.NEPALI to "हात सीधा क्यामेरा तर्फ देखाउनुहोस्",
            AppLanguage.HINDI to "हथेली को कैमरे के सामने सीधा रखें"
        ),
        "guidance.dark" to mapOf(
            AppLanguage.ENGLISH to "Too dark — add more light",
            AppLanguage.NEPALI to "धेरै अँध्यारो छ — थप उज्यालो चाहिन्छ",
            AppLanguage.HINDI to "बहुत अंधेरा है — रोशनी बढ़ाएं"
        ),
        "guidance.glare" to mapOf(
            AppLanguage.ENGLISH to "Angle palm away from glare",
            AppLanguage.NEPALI to "सिधा प्रकाशबाट हात अलि ढल्काउनुहोस्",
            AppLanguage.HINDI to "सीधी रोशनी से हथेली थोड़ा घुमाएं"
        ),
        "guidance.steady" to mapOf(
            AppLanguage.ENGLISH to "Hold steady...",
            AppLanguage.NEPALI to "हात स्थिर राख्नुहोस्...",
            AppLanguage.HINDI to "स्थिर रखें..."
        ),
        "guidance.ready" to mapOf(
            AppLanguage.ENGLISH to "Capturing palm lines...",
            AppLanguage.NEPALI to "हातका रेखाहरू खिचिँदैछ...",
            AppLanguage.HINDI to "हथेली की रेखाएं कैप्चर हो रही हैं..."
        ),
        "line.heart" to mapOf(
            AppLanguage.ENGLISH to "Heart Line",
            AppLanguage.NEPALI to "हृदय रेखा",
            AppLanguage.HINDI to "हृदय रेखा"
        ),
        "line.head" to mapOf(
            AppLanguage.ENGLISH to "Head Line",
            AppLanguage.NEPALI to "मस्तिष्क रेखा",
            AppLanguage.HINDI to "मस्तिष्क रेखा"
        ),
        "line.life" to mapOf(
            AppLanguage.ENGLISH to "Life Line",
            AppLanguage.NEPALI to "जीवन रेखा",
            AppLanguage.HINDI to "जीवन रेखा"
        ),
        "line.fate" to mapOf(
            AppLanguage.ENGLISH to "Fate Line",
            AppLanguage.NEPALI to "भाग्य रेखा",
            AppLanguage.HINDI to "भाग्य रेखा"
        ),
        "button.capture" to mapOf(
            AppLanguage.ENGLISH to "Capture",
            AppLanguage.NEPALI to "खिच्नुहोस्",
            AppLanguage.HINDI to "कैप्चर"
        ),
        "button.retake" to mapOf(
            AppLanguage.ENGLISH to "Retake",
            AppLanguage.NEPALI to "पुनः खिच्नुहोस्",
            AppLanguage.HINDI to "फिर से लें"
        ),
        "sheet.title" to mapOf(
            AppLanguage.ENGLISH to "Samudrika Palm Analysis",
            AppLanguage.NEPALI to "सामुद्रिक हस्तरेखा विश्लेषण",
            AppLanguage.HINDI to "सामुद्रिक हस्तरेखा विश्लेषण"
        ),
        "status.processing" to mapOf(
            AppLanguage.ENGLISH to "Extracting Sub-Pixel Ridges...",
            AppLanguage.NEPALI to "सुक्ष्म रेखाहरू प्रशोधन हुँदैछ...",
            AppLanguage.HINDI to "सूक्ष्म रेखाओं का विश्लेषण हो रहा है..."
        )
    )

    fun tr(key: String, lang: AppLanguage = currentLanguage): String {
        return table[key]?.get(lang) ?: table[key]?.get(AppLanguage.ENGLISH) ?: key
    }
}
