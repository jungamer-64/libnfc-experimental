#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(_WIN32)
#include <io.h>
#define dup _dup
#define dup2 _dup2
#define close _close
#define fileno _fileno
#else
#include <unistd.h>
#endif

#include "libnfc/log.h"
#include "libnfc/nfc-internal.h"

#define CHECK(condition, ...)                                           \
  do {                                                                  \
    if (!(condition)) {                                                 \
      fprintf(stderr, "FAIL:%d: ", __LINE__);                           \
      fprintf(stderr, __VA_ARGS__);                                     \
      fputc('\n', stderr);                                              \
      return 1;                                                         \
    }                                                                   \
  } while (0)

static int
capture_log_output(nfc_context *context, uint32_t log_level,
                   char *buffer, size_t buffer_size)
{
  FILE *tmp = tmpfile();
  int saved_stderr = -1;
  size_t read_len;

  if (!tmp || buffer_size == 0) {
    if (tmp) {
      fclose(tmp);
    }
    return 0;
  }

  context->log_level = log_level;
  log_init(context);

  saved_stderr = dup(fileno(stderr));
  if (saved_stderr < 0) {
    fclose(tmp);
    return 0;
  }

  fflush(stderr);
  if (dup2(fileno(tmp), fileno(stderr)) < 0) {
    close(saved_stderr);
    fclose(tmp);
    return 0;
  }

  log_put(NFC_LOG_GROUP_GENERAL, "libnfc.shim", NFC_LOG_PRIORITY_DEBUG,
          "value=%d", 7);
  fflush(stderr);

  if (dup2(saved_stderr, fileno(stderr)) < 0) {
    close(saved_stderr);
    fclose(tmp);
    return 0;
  }
  close(saved_stderr);
  log_exit();

  rewind(tmp);
  read_len = fread(buffer, 1, buffer_size - 1, tmp);
  buffer[read_len] = '\0';
  fclose(tmp);
  return 1;
}

int
main(void)
{
  nfc_context *context = nfc_context_alloc_defaults();
  char output[128];

  CHECK(context != NULL, "nfc_context_alloc_defaults() should succeed");

  CHECK(capture_log_output(context, NFC_LOG_PRIORITY_ERROR, output, sizeof(output)),
        "capturing filtered log output should succeed");
  CHECK(output[0] == '\0',
        "debug output should be filtered when log_level only enables errors");

  CHECK(capture_log_output(context, NFC_LOG_PRIORITY_DEBUG, output, sizeof(output)),
        "capturing formatted log output should succeed");
  CHECK(strcmp(output, "debug\tlibnfc.shim\tvalue=7\n") == 0,
        "formatted C log should match the Rust-rendered stderr line, got: %s",
        output);

  nfc_context_free(context);
  return 0;
}
