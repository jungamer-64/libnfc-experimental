#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#include <nfc/nfc.h>

#include "libnfc/nfc-internal.h"

#define TEST_ALPHA_DRIVER_NAME "test_alpha"
#define TEST_BETA_DRIVER_NAME "test_beta"
#define TEST_ALPHA_DEVICE_NAME "test-alpha-device"
#define TEST_BETA_DEVICE_NAME "test-beta-device"

#define CHECK(condition, ...)                                           \
  do {                                                                  \
    if (!(condition)) {                                                 \
      fprintf(stderr, "FAIL:%d: ", __LINE__);                           \
      fprintf(stderr, __VA_ARGS__);                                     \
      fputc('\n', stderr);                                              \
      return 1;                                                         \
    }                                                                   \
  } while (0)

typedef struct {
  int alpha_scan_calls;
  int beta_scan_calls;
  int alpha_open_calls;
  int beta_open_calls;
  int alpha_close_calls;
  int beta_close_calls;
} fake_driver_state;

static fake_driver_state state;
static const struct nfc_driver TEST_ALPHA_DRIVER;
static const struct nfc_driver TEST_BETA_DRIVER;

static void
reset_fake_driver_state(void)
{
  memset(&state, 0, sizeof(state));
}

static int
copy_c_string(char *dst, size_t dst_size, const char *src)
{
  const size_t length = strlen(src);

  if (length >= dst_size) {
    return 0;
  }

  memcpy(dst, src, length + 1);
  return 1;
}

static size_t
scan_fake_driver(nfc_connstring connstrings[], size_t connstrings_len,
                 const char *connstring)
{
  if (connstrings_len == 0) {
    return 0;
  }

  if (!copy_c_string(connstrings[0], sizeof(nfc_connstring), connstring)) {
    return 0;
  }

  return 1;
}

static nfc_device *
open_fake_driver(const nfc_context *context, const nfc_connstring connstring,
                 const char *expected_connstring, const struct nfc_driver *driver,
                 const char *device_name)
{
  nfc_device *device;

  if (strcmp(connstring, expected_connstring) != 0) {
    return NULL;
  }

  device = calloc(1, sizeof(*device));
  if (!device) {
    return NULL;
  }

  device->context = context;
  device->driver = driver;
  if (!copy_c_string(device->connstring, sizeof(device->connstring), connstring)) {
    free(device);
    return NULL;
  }
  if (!copy_c_string(device->name, sizeof(device->name), device_name)) {
    free(device);
    return NULL;
  }

  return device;
}

static size_t
alpha_scan(const nfc_context *context, nfc_connstring connstrings[],
           const size_t connstrings_len)
{
  (void)context;
  state.alpha_scan_calls++;
  return scan_fake_driver(connstrings, connstrings_len, TEST_ALPHA_DRIVER_NAME);
}

static size_t
beta_scan(const nfc_context *context, nfc_connstring connstrings[],
          const size_t connstrings_len)
{
  (void)context;
  state.beta_scan_calls++;
  return scan_fake_driver(connstrings, connstrings_len, TEST_BETA_DRIVER_NAME);
}

static void
alpha_close(nfc_device *pnd)
{
  state.alpha_close_calls++;
  free(pnd->driver_data);
  free(pnd);
}

static void
beta_close(nfc_device *pnd)
{
  state.beta_close_calls++;
  free(pnd->driver_data);
  free(pnd);
}

static nfc_device *
alpha_open(const nfc_context *context, const nfc_connstring connstring)
{
  state.alpha_open_calls++;
  return open_fake_driver(context, connstring, TEST_ALPHA_DRIVER_NAME,
                          &TEST_ALPHA_DRIVER, TEST_ALPHA_DEVICE_NAME);
}

static nfc_device *
beta_open(const nfc_context *context, const nfc_connstring connstring)
{
  state.beta_open_calls++;
  return open_fake_driver(context, connstring, TEST_BETA_DRIVER_NAME,
                          &TEST_BETA_DRIVER, TEST_BETA_DEVICE_NAME);
}

static const struct nfc_driver TEST_ALPHA_DRIVER = {
  .name = TEST_ALPHA_DRIVER_NAME,
  .scan_type = NOT_INTRUSIVE,
  .scan = alpha_scan,
  .open = alpha_open,
  .close = alpha_close,
};

static const struct nfc_driver TEST_BETA_DRIVER = {
  .name = TEST_BETA_DRIVER_NAME,
  .scan_type = NOT_INTRUSIVE,
  .scan = beta_scan,
  .open = beta_open,
  .close = beta_close,
};

int
main(void)
{
  nfc_context *context = NULL;
  nfc_connstring alpha_connstring;
  nfc_connstring beta_connstring;
  nfc_connstring discovered[2];
  nfc_device *device;
  size_t found;

  memset(alpha_connstring, 0, sizeof(alpha_connstring));
  memset(beta_connstring, 0, sizeof(beta_connstring));
  memset(discovered, 0, sizeof(discovered));
  reset_fake_driver_state();
  CHECK(copy_c_string(alpha_connstring, sizeof(alpha_connstring),
                      TEST_ALPHA_DRIVER_NAME),
        "alpha connstring should fit in the test buffer");
  CHECK(copy_c_string(beta_connstring, sizeof(beta_connstring),
                      TEST_BETA_DRIVER_NAME),
        "beta connstring should fit in the test buffer");

  CHECK(nfc_register_driver(&TEST_ALPHA_DRIVER) == NFC_SUCCESS,
        "registering alpha driver should succeed");
  CHECK(nfc_register_driver(&TEST_BETA_DRIVER) == NFC_SUCCESS,
        "registering beta driver should succeed");

  nfc_init(&context);
  CHECK(context != NULL, "nfc_init() should allocate a context");

  found = nfc_list_devices(context, discovered, 2);
  CHECK(found == 2, "nfc_list_devices() should find both fake drivers, got %lu",
        (unsigned long)found);
  CHECK(strcmp(discovered[0], TEST_BETA_DRIVER_NAME) == 0,
        "last registered driver should scan first");
  CHECK(strcmp(discovered[1], TEST_ALPHA_DRIVER_NAME) == 0,
        "first registered driver should scan second");
  CHECK(state.alpha_scan_calls == 1, "alpha scan should run exactly once");
  CHECK(state.beta_scan_calls == 1, "beta scan should run exactly once");

  reset_fake_driver_state();
  device = nfc_open(context, NULL);
  CHECK(device != NULL, "nfc_open(context, NULL) should open the first discovered driver");
  CHECK(strcmp(nfc_device_get_connstring(device), TEST_BETA_DRIVER_NAME) == 0,
        "nfc_open(context, NULL) should select beta first");
  CHECK(strcmp(nfc_device_get_name(device), TEST_BETA_DEVICE_NAME) == 0,
        "beta device should expose the fake device name");
  CHECK(state.beta_scan_calls == 1,
        "beta scan should provide the fallback connstring");
  CHECK(state.alpha_scan_calls == 0,
        "alpha scan should not run once the fallback slot is filled");
  CHECK(state.beta_open_calls == 1, "beta open should run once for fallback open");
  CHECK(state.alpha_open_calls == 0,
        "alpha open should not run when beta already matched the fallback open");
  nfc_close(device);
  CHECK(state.beta_close_calls == 1, "nfc_close() should call beta close");
  CHECK(state.alpha_close_calls == 0, "alpha close should remain untouched");

  reset_fake_driver_state();
  device = nfc_open(context, alpha_connstring);
  CHECK(device != NULL, "nfc_open(context, explicit alpha) should succeed");
  CHECK(strcmp(nfc_device_get_connstring(device), TEST_ALPHA_DRIVER_NAME) == 0,
        "explicit alpha open should return the alpha connstring");
  CHECK(strcmp(nfc_device_get_name(device), TEST_ALPHA_DEVICE_NAME) == 0,
        "explicit alpha open should return the alpha device name");
  CHECK(state.alpha_open_calls == 1, "alpha open should run once");
  CHECK(state.beta_open_calls == 0,
        "beta open should not run for an alpha connstring");
  nfc_close(device);
  CHECK(state.alpha_close_calls == 1, "nfc_close() should call alpha close");
  CHECK(state.beta_close_calls == 0, "beta close should remain untouched");

  nfc_exit(context);
  context = NULL;

  reset_fake_driver_state();
  nfc_init(&context);
  CHECK(context != NULL, "nfc_init() should allocate a fresh context after nfc_exit()");
  device = nfc_open(context, beta_connstring);
  CHECK(device == NULL,
        "nfc_exit() should clear custom registered drivers before the next nfc_init()");
  CHECK(state.alpha_open_calls == 0 && state.beta_open_calls == 0,
        "cleared fake drivers should not receive open callbacks after re-init");
  nfc_exit(context);

  return 0;
}
