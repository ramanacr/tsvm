#ifndef TSVM_C_API_H_
#define TSVM_C_API_H_

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32) && defined(TSVM_USE_DLL)
#define TSVM_API __declspec(dllimport)
#else
#define TSVM_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef enum tsvm_status {
  TSVM_STATUS_OK = 0,
  TSVM_STATUS_INVALID_ARGUMENT = 1,
  TSVM_STATUS_INVALID_UTF8 = 2,
  TSVM_STATUS_COMPILE_ERROR = 3,
  TSVM_STATUS_VERIFY_ERROR = 4,
  TSVM_STATUS_RUNTIME_ERROR = 5,
  TSVM_STATUS_INTERNAL_ERROR = 6,
} tsvm_status;

typedef struct tsvm_result tsvm_result;
typedef struct tsvm_page_session tsvm_page_session;

typedef enum tsvm_script_policy {
  TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT = 0,
  TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT = 1,
} tsvm_script_policy;

typedef struct tsvm_cache_stats {
  size_t hits;
  size_t misses;
  size_t evictions;
  size_t entries;
} tsvm_cache_stats;

TSVM_API tsvm_status tsvm_execute_utf8(const unsigned char* source,
                                       size_t source_len,
                                       tsvm_result** out_result);
TSVM_API const unsigned char* tsvm_result_json(const tsvm_result* result,
                                               size_t* out_len);
TSVM_API void tsvm_result_free(tsvm_result* result);
TSVM_API tsvm_status tsvm_page_session_create(size_t cache_capacity,
                                               tsvm_page_session** out_session);
TSVM_API tsvm_status tsvm_page_session_execute_utf8(
    tsvm_page_session* session,
    const unsigned char* source,
    size_t source_len,
    tsvm_script_policy policy,
    tsvm_result** out_result);
TSVM_API tsvm_status tsvm_page_session_cache_stats(
    const tsvm_page_session* session,
    tsvm_cache_stats* out_stats);
TSVM_API void tsvm_page_session_free(tsvm_page_session* session);
TSVM_API uint32_t tsvm_abi_version(void);

#ifdef __cplusplus
}
#endif

#endif  // TSVM_C_API_H_
