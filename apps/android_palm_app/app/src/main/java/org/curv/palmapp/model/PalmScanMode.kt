package org.curv.palmapp.model

/**
 * Capture protocol selector — Mode A (single still) vs enriched 3D scan modes.
 */
enum class PalmScanMode(val key: String, val displayName: String, val subtitle: String) {
    STILL_PHOTO("still", "Still Photo", "Classic single capture"),
    ORBIT_SCAN("orbit", "Orbit Scan", "Tilt hand for mount height map"),
    CLOSE_UP_MOSAIC("closeup", "Close-up Lines", "Fine crease detail tiles"),
    FINGERTIP_SCAN("fingertip", "Fingertips", "Dermatoglyphic pattern class");

    companion object {
        val all = entries.toList()
    }
}
