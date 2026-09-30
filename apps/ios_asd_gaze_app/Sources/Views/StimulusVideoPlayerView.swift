import SwiftUI
import AVKit

// MARK: - AVPlayer UIViewControllerRepresentable

/// Hardware-accelerated native video player wrapper for SwiftUI.
/// Configured specifically for frame-accurate stimulus presentation.
///
/// Exposes the authoritative AVPlayer media clock (10 Hz periodic observer) so that
/// time-coded AOI windows and per-clip expected-vs-recorded scoring align to actual
/// playback time rather than wall-clock accumulation. `onMediaUpdate` fires on the
/// main queue with the current media time in seconds (wraps to ~0 on loop).
public struct StimulusVideoPlayerView: UIViewControllerRepresentable {
    public let videoURL: URL
    public let isLooping: Bool
    public var onEnded: (() -> Void)?
    public var onMediaUpdate: ((TimeInterval) -> Void)?

    public init(
        videoURL: URL,
        isLooping: Bool = true,
        onEnded: (() -> Void)? = nil,
        onMediaUpdate: ((TimeInterval) -> Void)? = nil
    ) {
        self.videoURL = videoURL
        self.isLooping = isLooping
        self.onEnded = onEnded
        self.onMediaUpdate = onMediaUpdate
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

        context.coordinator.attach(player: player)

        player.play()
        return controller
    }

    public func updateUIViewController(_ uiViewController: AVPlayerViewController, context: Context) {
        context.coordinator.parent = self

        guard let player = uiViewController.player else { return }

        // Check if URL changed
        let currentAssetURL = (player.currentItem?.asset as? AVURLAsset)?.url
        if currentAssetURL != videoURL {
            let newItem = AVPlayerItem(url: videoURL)
            context.coordinator.attach(player: player, replacingItemWith: newItem)
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
        private weak var player: AVPlayer?
        private var timeObserverToken: Any?
        private var observedItem: AVPlayerItem?

        init(_ parent: StimulusVideoPlayerView) {
            self.parent = parent
        }

        /// Registers playback-end and periodic media-clock observers for the player's
        /// current item; tears down prior observers when the item is replaced mid-session.
        func attach(player: AVPlayer, replacingItemWith newItem: AVPlayerItem? = nil) {
            self.player = player
            if let token = timeObserverToken {
                player.removeTimeObserver(token)
                timeObserverToken = nil
            }
            if let oldItem = observedItem {
                NotificationCenter.default.removeObserver(self, name: .AVPlayerItemDidPlayToEndTime, object: oldItem)
                observedItem = nil
            }

            let currentItem = newItem ?? player.currentItem
            guard let currentItem = currentItem else { return }

            observedItem = currentItem
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(playerItemDidReachEnd(notification:)),
                name: .AVPlayerItemDidPlayToEndTime,
                object: currentItem
            )

            timeObserverToken = player.addPeriodicTimeObserver(
                forInterval: CMTime(seconds: 0.1, preferredTimescale: 600),
                queue: .main
            ) { [weak self] time in
                guard let self = self else { return }
                self.parent.onMediaUpdate?(time.seconds)
            }
        }

        @objc func playerItemDidReachEnd(notification: Notification) {
            if self.parent.isLooping {
                if let playerItem = notification.object as? AVPlayerItem {
                    playerItem.seek(to: .zero, completionHandler: nil)
                }
            }
            self.parent.onEnded?()
        }

        deinit {
            if let token = timeObserverToken {
                player?.removeTimeObserver(token)
            }
            if let item = observedItem {
                NotificationCenter.default.removeObserver(self, name: .AVPlayerItemDidPlayToEndTime, object: item)
            }
        }
    }
}
