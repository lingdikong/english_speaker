import MediaPlayer
import UIKit

/// 锁屏 / 控制中心：MPNowPlayingInfoCenter + MPRemoteCommandCenter。
/// 上一首 / 下一首映射为上一句 / 下一句（UI_SPEC §1）。
@MainActor
final class NowPlayingCenter {
    private weak var store: Store?
    private var artworkFor: String?
    private var artwork: MPMediaItemArtwork?
    private var lastElapsedPush = Date.distantPast

    init(store: Store) {
        self.store = store
        let cc = MPRemoteCommandCenter.shared()
        cc.playCommand.addTarget { [weak self] _ in self?.store?.resume(); return .success }
        cc.pauseCommand.addTarget { [weak self] _ in self?.store?.pause(); return .success }
        cc.togglePlayPauseCommand.addTarget { [weak self] _ in self?.store?.togglePlay(); return .success }
        cc.nextTrackCommand.addTarget { [weak self] _ in self?.store?.nextSentence(); return .success }
        cc.previousTrackCommand.addTarget { [weak self] _ in self?.store?.previousSentence(); return .success }
        cc.changePlaybackPositionCommand.addTarget { [weak self] e in
            guard let e = e as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            self?.store?.seek(to: UInt64(max(0, e.positionTime * 1000)))
            return .success
        }
        cc.changePlaybackRateCommand.supportedPlaybackRates = Store.rates.map { NSNumber(value: $0) }
        cc.changePlaybackRateCommand.addTarget { [weak self] e in
            guard let e = e as? MPChangePlaybackRateCommandEvent else { return .commandFailed }
            self?.store?.core?.setRate(rate: e.playbackRate)
            return .success
        }
    }

    /// 清空锁屏 / 控制中心 / 灵动岛上的播放信息。
    /// 只要 nowPlayingInfo 还在，系统就会一直显示本 App 的播放指示器与媒体控件。
    func clear() {
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        MPNowPlayingInfoCenter.default().playbackState = .stopped
        artwork = nil
        artworkFor = nil
        lastElapsedPush = .distantPast
    }

    func update() {
        guard let store, let a = store.current else { clear(); return }
        // 播完 / 没加载文章：不该再占用灵动岛和锁屏控件
        if store.player.status == .ended || store.player.status == .idle { clear(); return }
        if artworkFor != a.id, let image = CoverRenderer.image(title: a.title) {
            artworkFor = a.id
            artwork = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
        }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: a.title,
            MPMediaItemPropertyArtist: store.voiceLabel(a),
            MPMediaItemPropertyAlbumTitle: "English Speaker",
            MPMediaItemPropertyPlaybackDuration: Double(store.durationMs) / 1000,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: Double(store.positionMs) / 1000,
            MPNowPlayingInfoPropertyPlaybackRate: store.player.status == .playing ? Double(store.player.rate) : 0,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
        ]
        if let artwork { info[MPMediaItemPropertyArtwork] = artwork }
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
        // 必须是准确的状态：停在 .playing 的话，锁屏上会一直显示“正在播放”。
        MPNowPlayingInfoCenter.default().playbackState = {
            switch store.player.status {
            case .playing, .buffering: return .playing
            case .paused: return .paused
            default: return .stopped
            }
        }()
        lastElapsedPush = Date()
    }

    /// 系统会按 rate 自行外推进度；只需偶尔校准（时长未定稿时也会变化）。
    func updateElapsedIfNeeded() {
        if Date().timeIntervalSince(lastElapsedPush) > 5 { update() }
    }
}
