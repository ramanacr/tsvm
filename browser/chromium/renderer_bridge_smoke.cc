#include "tsvm_renderer_bridge.h"

#include <utility>

int main() {
  auto created = tsvm::chromium::PageSession::Create(1);
  if (created.status != TSVM_STATUS_OK || !created.session.is_valid()) {
    return 1;
  }

  auto session = std::move(created.session);
  const auto first = session.ExecuteInline(
      "console.log(150);", TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT);
  const auto second = session.ExecuteInline(
      "console.log(150);", TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT);
  if (first.json.empty() || second.json.empty()) {
    return 1;
  }

  const auto stats = session.CacheStats();
  if (stats.status != TSVM_STATUS_OK || stats.hits != 1 || stats.misses != 1 ||
      stats.evictions != 0 || stats.entries != 1) {
    return 1;
  }

  const auto blocked = session.ExecuteInline(
      "console.log(151);", TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT);
  if (blocked.status != TSVM_STATUS_RUNTIME_ERROR || blocked.json.empty()) {
    return 1;
  }

  const auto after_block = session.CacheStats();
  return after_block.hits == 1 && after_block.misses == 1 &&
                 after_block.evictions == 0 && after_block.entries == 1
             ? 0
             : 1;
}
