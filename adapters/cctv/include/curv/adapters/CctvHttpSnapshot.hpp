#pragma once

#include "curv/Frame.hpp"
#include "curv/adapters/CctvFrameSource.hpp"
#include <chrono>
#include <cstdint>
#include <string>
#include <vector>

namespace CurvEngine::adapters {

/**
 * @brief Phase 3 HTTP snapshot puller for public CCTV JPEG/PNG endpoints.
 *
 * Supports:
 * - https?:// live cameras (libcurl)
 * - file:// and bare filesystem paths (offline / tests)
 * - mock://<camera_id> → bundled fixtures under data/fixtures/cctv/
 * - Optional disk cache for presentation-safe demos when the network flakes
 */
class CctvHttpSnapshot {
public:
    struct FetchStats {
        bool from_cache{false};
        bool from_mock{false};
        long http_status{0};
        std::chrono::milliseconds latency{0};
        std::string resolved_url;
        std::string error;
    };

    explicit CctvHttpSnapshot(CctvMetadata metadata = {});

    [[nodiscard]] const CctvMetadata& getMetadata() const noexcept;
    void setMetadata(const CctvMetadata& metadata);

    void setTimeout(std::chrono::milliseconds timeout) noexcept;
    [[nodiscard]] std::chrono::milliseconds timeout() const noexcept;

    void setUserAgent(std::string user_agent);
    [[nodiscard]] const std::string& userAgent() const noexcept;

    /**
     * @brief Directory for cached JPEG/PNG bodies (created on demand).
     * Empty disables caching.
     */
    void setCacheDir(std::string cache_dir);
    [[nodiscard]] const std::string& cacheDir() const noexcept;

    /**
     * @brief When true, serve from cache if present before attempting network.
     * Presentation default. Use setPreferCache(false) / --direct for live pulls.
     */
    void setPreferCache(bool enabled) noexcept;
    [[nodiscard]] bool preferCache() const noexcept;

    /**
     * @brief Fetch a snapshot URL into out_frame.
     * On network failure with a warm cache, returns the cached frame and sets stats.from_cache.
     */
    [[nodiscard]] bool fetch(const std::string& url, Frame& out_frame, FetchStats* stats = nullptr);

    /**
     * @brief Write raw image bytes into the cache for url (tests / presentation seeding).
     */
    [[nodiscard]] bool putCache(const std::string& url, const std::vector<std::uint8_t>& body);

    /**
     * @brief Resolve mock:// and registry camera IDs to concrete URLs or fixture paths.
     */
    [[nodiscard]] static std::string resolveCameraUrl(const std::string& camera_or_url);

private:
    [[nodiscard]] bool loadLocalPath(const std::string& path, Frame& out_frame, FetchStats& stats);
    [[nodiscard]] bool loadFromCache(const std::string& url, Frame& out_frame, FetchStats& stats);
    [[nodiscard]] bool httpGet(const std::string& url, std::vector<std::uint8_t>& body, FetchStats& stats);
    [[nodiscard]] bool writeCache(const std::string& url, const std::vector<std::uint8_t>& body) const;
    [[nodiscard]] std::string cachePathForUrl(const std::string& url) const;
    void populateMetadata(Frame& frame) const;

    CctvMetadata metadata_;
    std::chrono::milliseconds timeout_{std::chrono::seconds(10)};
    std::string user_agent_{"vision-perception/0.1 (+https://github.com/vision-perception; Phase3 snapshot)"};
    std::string cache_dir_;
    bool prefer_cache_{true};
};

} // namespace CurvEngine::adapters
