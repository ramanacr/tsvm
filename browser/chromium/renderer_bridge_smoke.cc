#include "tsvm_renderer_bridge.h"

int main() {
  const auto result = tsvm::chromium::ExecuteSource("console.log(150);");
  return result.json.empty() ? 1 : 0;
}
