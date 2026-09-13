package org.curv.overlay.palm

/**
 * Standard anatomical palmar pads for clinical and anthropometric research.
 *
 * Provides bidirectional translation to/from traditional palmistry mount names
 * (Samudrika Shastra) while keeping the core scanner and biometrics domain-agnostic.
 */
enum class AnatomicalPad(
    val key: String,
    val landmarkAnchors: List<Int>,
    val astrologicalMountName: String,
    val scientificDisplayName: String
) {
    THENAR("thenar", listOf(1, 2, 5), "venus", "Thenar Eminence"),
    HYPOTHENAR("hypothenar", listOf(0, 17, 18), "luna", "Hypothenar Eminence"),
    RADIAL_INTERDIGITAL("radial_interdigital", listOf(5, 6, 7), "jupiter", "Radial Interdigital Pad (II)"),
    CENTRAL_INTERDIGITAL("central_interdigital", listOf(9, 10, 11), "saturn", "Central Interdigital Pad (III)"),
    RING_INTERDIGITAL("ring_interdigital", listOf(13, 14, 15), "apollo", "Ring Interdigital Pad (IV)"),
    ULNAR_INTERDIGITAL("ulnar_interdigital", listOf(17, 18, 19), "mercury", "Ulnar Interdigital Pad (V)"),
    PALMAR_FOSSA("palmar_fossa", listOf(5, 9, 13, 17), "mars", "Palmar Fossa (Mid-Palm)");

    companion object {
        fun fromKey(key: String): AnatomicalPad? =
            entries.firstOrNull { it.key.equals(key, ignoreCase = true) }

        fun fromAstrologicalMount(name: String): AnatomicalPad? {
            return when (name.lowercase()) {
                "venus" -> THENAR
                "luna", "moon" -> HYPOTHENAR
                "jupiter" -> RADIAL_INTERDIGITAL
                "saturn" -> CENTRAL_INTERDIGITAL
                "apollo", "sun" -> RING_INTERDIGITAL
                "mercury" -> ULNAR_INTERDIGITAL
                "mars", "plain_of_mars" -> PALMAR_FOSSA
                else -> null
            }
        }
    }
}
