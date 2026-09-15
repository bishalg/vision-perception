import SwiftUI
import WidgetKit
import CareActivityKit
#if canImport(ActivityKit)
import ActivityKit
#endif

@main
struct AsdGazeCareWidget: WidgetBundle {
    var body: some Widget {
        CareCountdownLiveActivity()
    }
}

struct CareCountdownLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: CareCountdownAttributes.self) { context in
            VStack(alignment: .leading, spacing: 4) {
                Text(context.state.title)
                    .font(.headline)
                Text(context.state.subtitle)
                    .font(.caption)
                Text(timerInterval: Date.now...context.state.endsAt, countsDown: true)
                    .font(.system(.title2, design: .rounded).monospacedDigit())
            }
            .padding()
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Text(context.state.title)
                        .font(.caption.weight(.bold))
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Text(timerInterval: Date.now...context.state.endsAt, countsDown: true)
                        .monospacedDigit()
                        .font(.caption)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    Text(context.state.subtitle)
                        .font(.caption2)
                }
            } compactLeading: {
                Image(systemName: "pills.fill")
            } compactTrailing: {
                Text(timerInterval: Date.now...context.state.endsAt, countsDown: true)
                    .monospacedDigit()
                    .frame(maxWidth: 48)
            } minimal: {
                Image(systemName: "bell.fill")
            }
        }
    }
}
