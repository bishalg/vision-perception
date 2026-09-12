package org.curv.palmapp.model

import android.content.Context
import org.json.JSONObject

/**
 * Internationalization (i18n) strings engine for Android Palm Perception app.
 * Supports English (en), Nepali (ne), and Hindi (hi).
 *
 * Single source of truth: `apps/mobile_overlay/shared/strings/palm_strings.json`
 * (wired as an `assets` source dir in app/build.gradle.kts) — the exact same
 * file the iOS app bundles as a resource. There is no per-app string table;
 * edit the JSON. Lookup falls back requested language → English → key itself.
 */
enum class AppLanguage(val code: String, val displayName: String) {
    ENGLISH("en", "English"),
    NEPALI("ne", "नेपाली"),
    HINDI("hi", "हिन्दी");

    companion object {
        fun fromCode(code: String): AppLanguage {
            for (lang in values()) {
                if (lang.code == code) return lang
            }
            return ENGLISH
        }
    }
}

object LocalizedStrings {
    var currentLanguage: AppLanguage = AppLanguage.ENGLISH

    @Volatile
    private var table: Map<String, Map<String, String>> = emptyMap()

    /**
     * Load the shared strings file from assets. Idempotent and thread-safe;
     * call once from [MainActivity.onCreate]. Until loaded, [tr] returns keys.
     */
    @Synchronized
    fun init(context: Context) {
        if (table.isNotEmpty()) return
        table = try {
            context.assets.open("palm_strings.json").bufferedReader(Charsets.UTF_8).use { reader ->
                val root = JSONObject(reader.readText())
                val outer = mutableMapOf<String, Map<String, String>>()
                val keys = root.keys()
                while (keys.hasNext()) {
                    val key = keys.next()
                    val langs = root.getJSONObject(key)
                    val inner = mutableMapOf<String, String>()
                    val langKeys = langs.keys()
                    while (langKeys.hasNext()) {
                        val lang = langKeys.next()
                        inner[lang] = langs.getString(lang)
                    }
                    outer[key] = inner
                }
                outer
            }
        } catch (_: Exception) {
            emptyMap()
        }
    }

    fun tr(key: String, lang: AppLanguage = currentLanguage): String {
        val entry = table[key] ?: return key
        return entry[lang.code] ?: entry["en"] ?: key
    }
}
