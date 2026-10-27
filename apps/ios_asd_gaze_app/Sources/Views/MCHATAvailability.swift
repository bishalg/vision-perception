import SwiftUI

/// H3: the M-CHAT-R/F questionnaire needs written permission from the instrument's
/// owners. It is compiled in only when the project is generated with
/// `MCHAT_LICENSED=1`, which defines `ASD_MCHAT_LICENSED`. Every presenter asks
/// this type, so the default build offers no questionnaire and says so.
/// Stored results and the history view stay available read-only.
public enum MCHATAvailability {
    public static var isLicensed: Bool {
        #if ASD_MCHAT_LICENSED
        true
        #else
        false
        #endif
    }

    public static var canStartQuestionnaire: Bool { isLicensed }

    /// Honest copy shown in place of the entry point; nil when licensed.
    public static var unavailableMessage: String? {
        isLicensed ? nil : "The M-CHAT-R/F questionnaire is not available in this version of the app. Any results saved earlier can still be viewed."
    }
}
