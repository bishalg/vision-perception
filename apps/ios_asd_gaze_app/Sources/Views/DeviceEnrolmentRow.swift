import SwiftUI

/// Settings row for registering this device with the clinic backend using the
/// one-time enrolment code from the clinic console (WP-A.4). Present only when the
/// configuration carries an `apiBaseURL`. Enrolment follows the clinic delivery opt-in (S-19):
/// the code field and Enrol are enabled only while it is on.
struct DeviceEnrolmentRow: View {
    @ObservedObject var syncService: ResearchKitSyncService
    @ObservedObject var optIns: FeatureOptIns = .shared
    @State private var code = ""
    @State private var isWorking = false
    @State private var message: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(Localizer.tr("enrolment_title"))
                .glassStyle(.headline)
            Text(statusText)
                .glassStyle(.subheadline)
                .accessibilityIdentifier("settings_device_enrolment_status")

            if needsCode {
                HStack {
                    TextField(Localizer.tr("enrolment_code_placeholder"), text: $code)
                        .textInputAutocapitalization(.characters)
                        .autocorrectionDisabled()
                        .textContentType(.oneTimeCode)
                        .disabled(!optIns.clinicDelivery)
                        .accessibilityIdentifier("settings_device_enrolment_code")
                    Button(Localizer.tr("enrolment_submit")) { enrol() }
                        .disabled(!optIns.clinicDelivery || isWorking || code.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("settings_device_enrolment_submit")
                }
                if !optIns.clinicDelivery {
                    Text(Localizer.tr("enrolment_needs_optin"))
                        .glassStyle(.subheadline)
                        .accessibilityIdentifier("settings_device_enrolment_optin_off")
                }
            }
            if let message {
                Text(message).glassStyle(.subheadline)
            }
        }
    }

    private var needsCode: Bool {
        switch syncService.deviceRegistration {
        case .unregistered, .needsEnrolment: return true
        case .registered, .unavailable: return false
        }
    }

    private var statusText: String {
        switch syncService.deviceRegistration {
        case .unavailable: return Localizer.tr("enrolment_status_unavailable")
        case .unregistered: return Localizer.tr("enrolment_status_unregistered")
        case .registered: return Localizer.tr("enrolment_status_registered")
        case .needsEnrolment: return Localizer.tr("enrolment_status_expired")
        }
    }

    private func enrol() {
        guard optIns.clinicDelivery else { return }
        isWorking = true
        message = nil
        let entered = code
        Task {
            do {
                try await syncService.registerDevice(enrolCode: entered)
                code = ""
                message = nil
            } catch {
                message = Localizer.tr("enrolment_code_rejected")
            }
            isWorking = false
        }
    }
}
