#include "tsvm_renderer_bridge.h"

#include <cstddef>
#include <utility>

namespace tsvm::chromium {
namespace {

class ResultHandle {
 public:
  explicit ResultHandle(tsvm_result* result) : result_(result) {}

  ResultHandle(const ResultHandle&) = delete;
  ResultHandle& operator=(const ResultHandle&) = delete;

  ~ResultHandle() { tsvm_result_free(result_); }

  [[nodiscard]] const tsvm_result* get() const { return result_; }

 private:
  tsvm_result* result_;
};

std::string CopyJson(const tsvm_result* result) {
  if (result == nullptr) {
    return {};
  }

  std::size_t length = 0;
  const auto* bytes = tsvm_result_json(result, &length);
  if (bytes == nullptr) {
    return {};
  }
  return {reinterpret_cast<const char*>(bytes), length};
}

}  // namespace

PageSession::PageSession(tsvm_page_session* session) : session_(session) {}

PageSessionCreation PageSession::Create(std::size_t cache_capacity) {
  tsvm_page_session* session = nullptr;
  const auto status = tsvm_page_session_create(cache_capacity, &session);
  return {status, PageSession(session)};
}

PageSession::PageSession(PageSession&& other) noexcept
    : session_(std::exchange(other.session_, nullptr)) {}

PageSession& PageSession::operator=(PageSession&& other) noexcept {
  if (this != &other) {
    tsvm_page_session_free(session_);
    session_ = std::exchange(other.session_, nullptr);
  }
  return *this;
}

PageSession::~PageSession() { tsvm_page_session_free(session_); }

bool PageSession::is_valid() const { return session_ != nullptr; }

ExecutionResult PageSession::ExecuteInline(std::string_view source,
                                           tsvm_script_policy policy) {
  if (!is_valid()) {
    return {TSVM_STATUS_INVALID_ARGUMENT, {}};
  }

  tsvm_result* raw_result = nullptr;
  const auto status = tsvm_page_session_execute_utf8(
      session_, reinterpret_cast<const unsigned char*>(source.data()), source.size(), policy,
      &raw_result);
  const ResultHandle result(raw_result);
  return {status, CopyJson(result.get())};
}

PageSessionCacheStats PageSession::CacheStats() const {
  if (!is_valid()) {
    return {TSVM_STATUS_INVALID_ARGUMENT};
  }

  tsvm_cache_stats raw_stats{};
  const auto status = tsvm_page_session_cache_stats(session_, &raw_stats);
  if (status != TSVM_STATUS_OK) {
    return {status};
  }
  return {status, raw_stats.hits, raw_stats.misses, raw_stats.evictions,
          raw_stats.entries};
}

ExecutionResult ExecuteSource(std::string_view source) {
  tsvm_result* raw_result = nullptr;
  const auto status = tsvm_execute_utf8(
      reinterpret_cast<const unsigned char*>(source.data()), source.size(),
      &raw_result);
  const ResultHandle result(raw_result);
  return {status, CopyJson(result.get())};
}

}  // namespace tsvm::chromium
