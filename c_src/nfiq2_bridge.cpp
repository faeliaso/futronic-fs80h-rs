#include <nfiq2.hpp>

#include <cstdint>
#include <exception>
#include <memory>
#include <string>
#include <unordered_map>

namespace {

thread_local std::string last_error;

constexpr uint8_t EmptyOrLowContrast = 1;
constexpr uint8_t UniformImage = 1 << 1;
constexpr uint8_t TooFewMinutiae = 1 << 2;
constexpr uint8_t InsufficientForeground = 1 << 3;

struct Engine {
    explicit Engine(const char *model_info_path)
        : algorithm(NFIQ2::ModelInfo(model_info_path)) {}

    NFIQ2::Algorithm algorithm;
};

void set_error(const std::exception &exception) {
    last_error = exception.what();
}

void set_unknown_error() {
    last_error = "unknown C++ exception";
}

} // namespace

extern "C" {

int fs80h_nfiq2_create(const char *model_info_path, void **out) {
    if (model_info_path == nullptr || out == nullptr) {
        last_error = "model-info path and output pointer are required";
        return -1;
    }
    try {
        *out = new Engine(model_info_path);
        return 0;
    } catch (const std::exception &exception) {
        set_error(exception);
    } catch (...) {
        set_unknown_error();
    }
    *out = nullptr;
    return -1;
}

void fs80h_nfiq2_destroy(void *engine) {
    delete static_cast<Engine *>(engine);
}

int fs80h_nfiq2_assess(void *engine, const uint8_t *pixels, uint32_t pixels_len,
                       uint32_t width, uint32_t height, uint16_t ppi,
                       uint8_t *score, uint8_t *feedback) {
    if (engine == nullptr || pixels == nullptr || score == nullptr || feedback == nullptr) {
        last_error = "engine, image data, score, and feedback pointers are required";
        return -1;
    }
    try {
        const NFIQ2::FingerprintImageData image(pixels, pixels_len, width, height, 0, ppi);
        const auto algorithms =
            NFIQ2::QualityMeasures::computeNativeQualityMeasureAlgorithms(image);
        const auto actions = NFIQ2::QualityMeasures::getActionableQualityFeedback(algorithms);

        uint8_t flags = 0;
        // These thresholds are supplied by NFIQ2 and define when an actionable value calls for
        // a corrective capture. The first measure flags a near-white image; the remaining three
        // describe values that must meet their respective minima.
        if (actions.at(NFIQ2::Identifiers::ActionableQualityFeedback::EmptyImageOrContrastTooLow) >=
            NFIQ2::Thresholds::ActionableQualityFeedback::EmptyImageOrContrastTooLow) {
            flags |= EmptyOrLowContrast;
        }
        if (actions.at(NFIQ2::Identifiers::ActionableQualityFeedback::UniformImage) <
            NFIQ2::Thresholds::ActionableQualityFeedback::UniformImage) {
            flags |= UniformImage;
        }
        if (actions.at(NFIQ2::Identifiers::ActionableQualityFeedback::FingerprintImageWithMinutiae) <
            NFIQ2::Thresholds::ActionableQualityFeedback::FingerprintImageWithMinutiae) {
            flags |= TooFewMinutiae;
        }
        if (actions.at(NFIQ2::Identifiers::ActionableQualityFeedback::SufficientFingerprintForeground) <
            NFIQ2::Thresholds::ActionableQualityFeedback::SufficientFingerprintForeground) {
            flags |= InsufficientForeground;
        }

        const auto result = static_cast<Engine *>(engine)->algorithm
                                .computeUnifiedQualityScore(algorithms);
        *score = static_cast<uint8_t>(result);
        *feedback = flags;
        return 0;
    } catch (const NFIQ2::Exception &exception) {
        set_error(exception);
        if (exception.getErrorCode() == NFIQ2::ErrorCode::FJFX_CannotCreateFeatureSet ||
            exception.getErrorCode() == NFIQ2::ErrorCode::FJFX_NoFeatureSetCreated) {
            return 2;
        }
    } catch (const std::exception &exception) {
    } catch (...) {
        set_unknown_error();
    }
    return -1;
}

const char *fs80h_nfiq2_last_error() {
    return last_error.c_str();
}

} // extern "C"
