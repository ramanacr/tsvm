#ifndef TSVM_RENDERER_BRIDGE_H_
#define TSVM_RENDERER_BRIDGE_H_

#include <string>
#include <string_view>

#include "tsvm_c_api.h"

namespace tsvm::chromium {

struct ExecutionResult {
  tsvm_status status;
  std::string json;
};

ExecutionResult ExecuteSource(std::string_view source);

}  // namespace tsvm::chromium

#endif  // TSVM_RENDERER_BRIDGE_H_
