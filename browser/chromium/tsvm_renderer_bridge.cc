#include "tsvm_renderer_bridge.h"

#include <cstddef>

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

ExecutionResult ExecuteSource(std::string_view source) {
  tsvm_result* raw_result = nullptr;
  const auto status = tsvm_execute_utf8(
      reinterpret_cast<const unsigned char*>(source.data()), source.size(),
      &raw_result);
  const ResultHandle result(raw_result);
  return {status, CopyJson(result.get())};
}

}  // namespace tsvm::chromium
