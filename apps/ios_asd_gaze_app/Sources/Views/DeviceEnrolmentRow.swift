import SwiftUI

/// Settings row for registering this device with the clinic backend using the
/// one-time enrolment code from the clinic console (WP-A.4). Present only when the
/// study configuration carries an `apiBaseURL`.
struct DeviceEnrolmentRow: View {
    @ObservedObject var syncService: ResearchKitSyncService
    @State private var code = ""
    @State private var isWorking = false
    @State private var message: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Clinic device enrolment")
                .glassStyle(.headline)
            Text(statusText)
                .glassStyle(.subheadline)
                .accessibilityIdentifier("settings_device_enrolment_status")

            if needsCode {
                HStack {
                    TextField("Enrolment code", text: $code)
                        .textInputAutocapitalization(.characters)
                        .autocorrectionDisabled()
                        .textContentType(.oneTimeCode)
                        .accessibilityIdentifier("settings_device_enrolment_code")
                    Button("Enrol") { enrol() }
                        .disabled(isWorking || code.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("settings_device_enrolment_submit")
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
        case .unavailable: return "Uploads are not set up in this build."
        case .unregistered: return "Enter the code from your clinic to send sessions you have consented to share."
        case .registered: return "This device is enrolled. Consented sessions upload when online."
        case .needsEnrolment: return "Enrolment expired. Enter a new code from your clinic. Queued sessions stay on this device until then."
        }
    }

    private func enrol() {
        isWorking = true
        message = nil
        let entered = code
        Task {
            do {
                try await syncService.registerDevice(enrolCode: entered)
                code = ""
                message = nil
            } catch {
                message = "That code was not accepted. Check it and try again."
            }
            isWorking = false
        }
    }
}
