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

TSVM_API tsvm_status tsvm_execute_utf8(const unsigned char* source,
                                       size_t source_len,
                                       tsvm_result** out_result);
TSVM_API const unsigned char* tsvm_result_json(const tsvm_result* result,
                                               size_t* out_len);
TSVM_API void tsvm_result_free(tsvm_result* result);
TSVM_API uint32_t tsvm_abi_version(void);

#ifdef __cplusplus
}
#endif

#endif  // TSVM_C_API_H_
