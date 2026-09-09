#include "curv/adapters/CctvHttpSnapshot.hpp"
#include <curl/curl.h>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <opencv2/imgcodecs.hpp>
#include <sstream>
#include <unordered_map>

namespace CurvEngine::adapters {

namespace {

// libcurl requires a non-const char* signature for the write callback.
// NOLINTNEXTLINE(readability-non-const-parameter,misc-const-correctness)
std::size_t curlWriteCallback(char* ptr, std::size_t size, std::size_t nmemb, void* userdata) {
    const std::size_t total = size * nmemb;
    auto* out = static_cast<std::vector<std::uint8_t>*>(userdata);
    const auto* bytes = reinterpret_cast<const std::uint8_t*>(ptr);
    out->insert(out->end(), bytes, bytes + total);
    return total;
}

void ensureCurlGlobalInit() {
    static std::once_flag once;
    std::call_once(once, [] { curl_global_init(CURL_GLOBAL_DEFAULT); });
}

std::string stripScheme(const std::string& url, const std::string& scheme) {
    if (url.starts_with(scheme)) {
        return url.substr(scheme.size());
    }
    return url;
}

std::string fnv1aHex(const std::string& input) {
    std::uint64_t hash = 14695981039346656037ull;
    for (const char ch : input) {
        hash ^= static_cast<std::uint64_t>(static_cast<unsigned char>(ch));
        hash *= 1099511628211ull;
    }
    std::ostringstream oss;
    oss << std::hex << hash;
    return oss.str();
}

const std::unordered_map<std::string, std::string>& mockFixtureMap() {
    static const std::unordered_map<std::string, std::string> kMap = {
        {"caltrans_i80_day", "data/fixtures/cctv/day_clear/caltrans_i80_day.png"},
        {"caltrans_i80_drum", "data/fixtures/cctv/day_clear/caltrans_i80_day.png"},
        {"caltrans_i80_night", "data/fixtures/cctv/night_lowlux/caltrans_i80_night.png"},
        {"caltrans_i80_rain", "data/fixtures/cctv/weather_rain/caltrans_i80_rain_blur.png"},
        {"bhote_koshi_pier", "data/fixtures/cctv_flood/bhote_koshi_pier_sample.png"},
    };
    return kMap;
}

const std::unordered_map<std::string, std::string>& liveCameraRegistry() {
    static const std::unordered_map<std::string, std::string> kRegistry = {
        {"caltrans_i80_drum", "https://cwwp2.dot.ca.gov/data/d3/cctv/image/i80atdrumforebay/i80atdrumforebay.jpg"},
        {"caltrans_us101_sf", "https://cwwp2.dot.ca.gov/data/d4/cctv/image/us101cesarchavez/us101cesarchavez.jpg"},
        {"austin_congress_6th", "https://data.austintexas.gov/views/b4k4-adkb/files/sample_snap.jpg"},
    };
    return kRegistry;
}

} // namespace

CctvHttpSnapshot::CctvHttpSnapshot(CctvMetadata metadata) : metadata_(std::move(metadata)) {}

const CctvMetadata& CctvHttpSnapshot::getMetadata() const noexcept {
    return metadata_;
}

void CctvHttpSnapshot::setMetadata(const CctvMetadata& metadata) {
    metadata_ = metadata;
}

void CctvHttpSnapshot::setTimeout(std::chrono::milliseconds timeout) noexcept {
    timeout_ = timeout;
}

std::chrono::milliseconds CctvHttpSnapshot::timeout() const noexcept {
    return timeout_;
}

void CctvHttpSnapshot::setUserAgent(std::string user_agent) {
    user_agent_ = std::move(user_agent);
}

const std::string& CctvHttpSnapshot::userAgent() const noexcept {
    return user_agent_;
}

void CctvHttpSnapshot::setCacheDir(std::string cache_dir) {
    cache_dir_ = std::move(cache_dir);
}

const std::string& CctvHttpSnapshot::cacheDir() const noexcept {
    return cache_dir_;
}

void CctvHttpSnapshot::setPreferCache(bool enabled) noexcept {
    prefer_cache_ = enabled;
}

bool CctvHttpSnapshot::preferCache() const noexcept {
    return prefer_cache_;
}

std::string CctvHttpSnapshot::resolveCameraUrl(const std::string& camera_or_url) {
    if (camera_or_url.starts_with("mock://") || camera_or_url.starts_with("file://") ||
        camera_or_url.starts_with("http://") || camera_or_url.starts_with("https://")) {
        return camera_or_url;
    }
    if (std::filesystem::exists(camera_or_url)) {
        return std::string("file://") + camera_or_url;
    }
    const auto& live = liveCameraRegistry();
    const auto it = live.find(camera_or_url);
    if (it != live.end()) {
        return it->second;
    }
    const auto& mocks = mockFixtureMap();
    if (mocks.contains(camera_or_url)) {
        return std::string("mock://") + camera_or_url;
    }
    return camera_or_url;
}

std::string CctvHttpSnapshot::cachePathForUrl(const std::string& url) const {
    if (cache_dir_.empty()) {
        return {};
    }
    return (std::filesystem::path(cache_dir_) / (fnv1aHex(url) + ".bin")).string();
}

void CctvHttpSnapshot::populateMetadata(Frame& frame) const {
    if (frame.source_id.empty()) {
        frame.source_id = metadata_.camera_id.empty() ? "cctv_http" : metadata_.camera_id;
    }
    frame.metadata["camera_id"] = metadata_.camera_id;
    frame.metadata["name"] = metadata_.name;
    frame.metadata["agency"] = metadata_.agency;
    frame.metadata["latitude"] = std::to_string(metadata_.latitude);
    frame.metadata["longitude"] = std::to_string(metadata_.longitude);
    if (!metadata_.stream_or_snapshot_url.empty()) {
        frame.metadata["stream_or_snapshot_url"] = metadata_.stream_or_snapshot_url;
    }
}

bool CctvHttpSnapshot::loadLocalPath(const std::string& path, Frame& out_frame, FetchStats& stats) {
    const auto t0 = std::chrono::steady_clock::now();
    CctvFrameSource source(metadata_);
    if (!source.loadFromFile(path, out_frame)) {
        stats.error = "failed to load local path: " + path;
        out_frame = Frame();
        return false;
    }
    populateMetadata(out_frame);
    stats.latency = std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - t0);
    stats.resolved_url = path;
    return true;
}

bool CctvHttpSnapshot::loadFromCache(const std::string& url, Frame& out_frame, FetchStats& stats) {
    const std::string path = cachePathForUrl(url);
    if (path.empty() || !std::filesystem::exists(path)) {
        return false;
    }
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return false;
    }
    const std::vector<std::uint8_t> body((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    CctvFrameSource source(metadata_);
    if (!source.loadFromMemory(body, out_frame)) {
        return false;
    }
    populateMetadata(out_frame);
    stats.from_cache = true;
    stats.resolved_url = path;
    return true;
}

bool CctvHttpSnapshot::writeCache(const std::string& url, const std::vector<std::uint8_t>& body) const {
    if (cache_dir_.empty() || body.empty()) {
        return false;
    }
    std::error_code ec;
    std::filesystem::create_directories(cache_dir_, ec);
    const std::string path = cachePathForUrl(url);
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    if (!out) {
        return false;
    }
    out.write(reinterpret_cast<const char*>(body.data()), static_cast<std::streamsize>(body.size()));
    return static_cast<bool>(out);
}

bool CctvHttpSnapshot::putCache(const std::string& url, const std::vector<std::uint8_t>& body) {
    return writeCache(url, body);
}

bool CctvHttpSnapshot::httpGet(const std::string& url, std::vector<std::uint8_t>& body, FetchStats& stats) {
    ensureCurlGlobalInit();
    CURL* curl = curl_easy_init();
    if (curl == nullptr) {
        stats.error = "curl_easy_init failed";
        return false;
    }

    body.clear();
    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_USERAGENT, user_agent_.c_str());
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, curlWriteCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &body);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, static_cast<long>(timeout_.count()));
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT_MS, static_cast<long>(timeout_.count()));
    curl_easy_setopt(curl, CURLOPT_ACCEPT_ENCODING, "");

    const CURLcode rc = curl_easy_perform(curl);
    if (rc != CURLE_OK) {
        stats.error = curl_easy_strerror(rc);
        curl_easy_cleanup(curl);
        body.clear();
        return false;
    }

    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &stats.http_status);
    curl_easy_cleanup(curl);

    if (stats.http_status < 200 || stats.http_status >= 300 || body.empty()) {
        stats.error = "HTTP status " + std::to_string(stats.http_status) + " or empty body";
        body.clear();
        return false;
    }
    return true;
}

bool CctvHttpSnapshot::fetch(const std::string& url_in, Frame& out_frame, FetchStats* stats_out) {
    FetchStats stats;
    const std::string url = resolveCameraUrl(url_in);
    stats.resolved_url = url;
    const auto t0 = std::chrono::steady_clock::now();

    const auto finish = [&](bool ok) {
        stats.latency = std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - t0);
        if (stats_out != nullptr) {
            *stats_out = stats;
        }
        if (!ok) {
            out_frame = Frame();
        }
        return ok;
    };

    if (url.starts_with("mock://")) {
        const std::string key = stripScheme(url, "mock://");
        const auto& mocks = mockFixtureMap();
        const auto it = mocks.find(key);
        if (it == mocks.end()) {
            stats.error = "unknown mock camera: " + key;
            return finish(false);
        }
        stats.from_mock = true;
        if (!loadLocalPath(it->second, out_frame, stats)) {
            return finish(false);
        }
        return finish(true);
    }

    if (url.starts_with("file://")) {
        const std::string path = stripScheme(url, "file://");
        if (!loadLocalPath(path, out_frame, stats)) {
            return finish(false);
        }
        return finish(true);
    }

    if (prefer_cache_ && loadFromCache(url, out_frame, stats)) {
        return finish(true);
    }

    if (url.starts_with("http://") || url.starts_with("https://")) {
        std::vector<std::uint8_t> body;
        if (httpGet(url, body, stats)) {
            CctvFrameSource source(metadata_);
            if (!source.loadFromMemory(body, out_frame)) {
                stats.error = "decoded empty image from HTTP body";
                // Fall through to cache if available.
            } else {
                populateMetadata(out_frame);
                (void)writeCache(url, body);
                return finish(true);
            }
        }

        if (loadFromCache(url, out_frame, stats)) {
            if (stats.error.empty()) {
                stats.error = "served from cache after network failure";
            }
            return finish(true);
        }
        return finish(false);
    }

    // Bare path fallback
    if (std::filesystem::exists(url)) {
        if (!loadLocalPath(url, out_frame, stats)) {
            return finish(false);
        }
        return finish(true);
    }

    stats.error = "unsupported URL scheme: " + url;
    return finish(false);
}

} // namespace CurvEngine::adapters
