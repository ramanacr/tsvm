#ifndef TSVM_RENDERER_BRIDGE_H_
#define TSVM_RENDERER_BRIDGE_H_

#include <cstddef>
#include <string>
#include <string_view>

#include "tsvm_c_api.h"

namespace tsvm::chromium {

struct ExecutionResult {
  tsvm_status status;
  std::string json;
};

struct PageSessionCacheStats {
  tsvm_status status = TSVM_STATUS_INTERNAL_ERROR;
  std::size_t hits = 0;
  std::size_t misses = 0;
  std::size_t evictions = 0;
  std::size_t entries = 0;
};

struct PageSessionCreation;

class PageSession {
 public:
  static PageSessionCreation Create(std::size_t cache_capacity);

  PageSession(PageSession&& other) noexcept;
  PageSession& operator=(PageSession&& other) noexcept;
  PageSession(const PageSession&) = delete;
  PageSession& operator=(const PageSession&) = delete;
  ~PageSession();

  [[nodiscard]] bool is_valid() const;
  ExecutionResult ExecuteInline(std::string_view source,
                                tsvm_script_policy policy);
  PageSessionCacheStats CacheStats() const;

 private:
  explicit PageSession(tsvm_page_session* session);

  tsvm_page_session* session_ = nullptr;
};

struct PageSessionCreation {
  tsvm_status status;
  PageSession session;
};

ExecutionResult ExecuteSource(std::string_view source);

}  // namespace tsvm::chromium

#endif  // TSVM_RENDERER_BRIDGE_H_
