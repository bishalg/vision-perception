import SwiftUI
import AVKit

// MARK: - AVPlayer UIViewControllerRepresentable

/// Hardware-accelerated native video player wrapper for SwiftUI.
/// Configured specifically for frame-accurate stimulus presentation.
public struct StimulusVideoPlayerView: UIViewControllerRepresentable {
    public let videoURL: URL
    public let isLooping: Bool
    public var onEnded: (() -> Void)?

    public init(
        videoURL: URL,
        isLooping: Bool = true,
        onEnded: (() -> Void)? = nil
    ) {
        self.videoURL = videoURL
        self.isLooping = isLooping
        self.onEnded = onEnded
    }

    public func makeUIViewController(context: Context) -> AVPlayerViewController {
        let player = AVPlayer(url: videoURL)
        player.actionAtItemEnd = .none

        let controller = AVPlayerViewController()
        controller.player = player
        controller.showsPlaybackControls = false
        controller.videoGravity = .resizeAspectFill

        // Configure audio session for clinical stimulus playback
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)
        try? AVAudioSession.sharedInstance().setActive(true)

        if let currentItem = player.currentItem {
            NotificationCenter.default.addObserver(
                context.coordinator,
                selector: #selector(Coordinator.playerItemDidReachEnd(notification:)),
                name: .AVPlayerItemDidPlayToEndTime,
                object: currentItem
            )
        }

        player.play()
        return controller
    }

    public func updateUIViewController(_ uiViewController: AVPlayerViewController, context: Context) {
        context.coordinator.parent = self

        guard let player = uiViewController.player else { return }

        // Check if URL changed
        let currentAssetURL = (player.currentItem?.asset as? AVURLAsset)?.url
        if currentAssetURL != videoURL {
            NotificationCenter.default.removeObserver(context.coordinator)
            let newItem = AVPlayerItem(url: videoURL)
            NotificationCenter.default.addObserver(
                context.coordinator,
                selector: #selector(Coordinator.playerItemDidReachEnd(notification:)),
                name: .AVPlayerItemDidPlayToEndTime,
                object: newItem
            )
            player.replaceCurrentItem(with: newItem)
            player.play()
        } else if player.rate == 0 {
            player.play()
        }
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    public final class Coordinator: NSObject {
        var parent: StimulusVideoPlayerView

        init(_ parent: StimulusVideoPlayerView) {
            self.parent = parent
        }

        @objc func playerItemDidReachEnd(notification: Notification) {
            if self.parent.isLooping {
                if let playerItem = notification.object as? AVPlayerItem {
                    playerItem.seek(to: .zero, completionHandler: nil)
                }
            }
            self.parent.onEnded?()
        }
    }
}
