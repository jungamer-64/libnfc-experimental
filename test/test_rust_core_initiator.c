#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#include <nfc/nfc.h>

#include "libnfc/nfc-internal.h"

#define TEST_DRIVER_NAME "rust_core_initiator_test"
#define MAX_PROPERTY_CALLS 16
#define MAX_PASSIVE_CALLS 8
#define MAX_DEP_CALLS 8
#define MAX_PAYLOAD_LEN 16

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
  nfc_property property;
  bool value;
} property_bool_call;

typedef struct {
  nfc_property property;
  int value;
} property_int_call;

typedef struct {
  size_t property_bool_call_count;
  property_bool_call property_bool_calls[MAX_PROPERTY_CALLS];
  size_t property_int_call_count;
  property_int_call property_int_calls[MAX_PROPERTY_CALLS];
  int initiator_init_calls;
  int initiator_init_secure_element_calls;
  int poll_target_calls;
  int poll_target_return;
  int deselect_calls;
  int target_is_present_calls;
  int target_is_present_return;
  size_t passive_call_count;
  size_t passive_payload_lens[MAX_PASSIVE_CALLS];
  uint8_t passive_payloads[MAX_PASSIVE_CALLS][MAX_PAYLOAD_LEN];
  int passive_results[MAX_PASSIVE_CALLS];
  nfc_target passive_targets[MAX_PASSIVE_CALLS];
  size_t dep_call_count;
  int dep_timeouts[MAX_DEP_CALLS];
  int dep_results[MAX_DEP_CALLS];
} fake_state;

static fake_state state;
static const struct nfc_driver TEST_DRIVER;

static void
reset_fake_state(void)
{
  memset(&state, 0, sizeof(state));
}

static nfc_target
make_target(uint8_t marker)
{
  nfc_target target;

  memset(&target, 0, sizeof(target));
  target.nm.nmt = NMT_ISO14443A;
  target.nm.nbr = NBR_106;
  target.nti.nai.szUidLen = 1;
  target.nti.nai.abtUid[0] = marker;

  return target;
}

static nfc_device *
make_device(void)
{
  nfc_device *device = calloc(1, sizeof(*device));

  if (!device) {
    return NULL;
  }

  snprintf(device->connstring, sizeof(device->connstring), "%s", TEST_DRIVER_NAME);
  device->driver = &TEST_DRIVER;
  return device;
}

static void
destroy_device(nfc_device *device)
{
  free(device);
}

static int
fake_set_property_bool(nfc_device *pnd, const nfc_property property, const bool enable)
{
  const size_t index = state.property_bool_call_count++;

  if (index < MAX_PROPERTY_CALLS) {
    state.property_bool_calls[index].property = property;
    state.property_bool_calls[index].value = enable;
  }

  if (property == NP_INFINITE_SELECT) {
    pnd->bInfiniteSelect = enable;
  }

  return NFC_SUCCESS;
}

static int
fake_set_property_int(nfc_device *pnd, const nfc_property property, const int value)
{
  const size_t index = state.property_int_call_count++;
  (void)pnd;

  if (index < MAX_PROPERTY_CALLS) {
    state.property_int_calls[index].property = property;
    state.property_int_calls[index].value = value;
  }

  return NFC_SUCCESS;
}

static int
fake_initiator_init(nfc_device *pnd)
{
  (void)pnd;
  state.initiator_init_calls++;
  return 77;
}

static int
fake_initiator_init_secure_element(nfc_device *pnd)
{
  (void)pnd;
  state.initiator_init_secure_element_calls++;
  return 88;
}

static int
fake_get_supported_modulation(nfc_device *pnd, const nfc_mode mode,
                              const nfc_modulation_type **const supported_mt)
{
  static const nfc_modulation_type supported[] = {
    NMT_ISO14443A,
    NMT_FELICA,
    (nfc_modulation_type) 0,
  };

  (void)pnd;
  (void)mode;
  *supported_mt = supported;
  return NFC_SUCCESS;
}

static int
fake_get_supported_baud_rate(nfc_device *pnd, const nfc_mode mode,
                             const nfc_modulation_type nmt,
                             const nfc_baud_rate **const supported_br)
{
  static const nfc_baud_rate supported_106[] = {
    NBR_106,
    NBR_UNDEFINED,
  };
  static const nfc_baud_rate supported_212[] = {
    NBR_212,
    NBR_UNDEFINED,
  };

  (void)pnd;
  (void)mode;
  *supported_br = (nmt == NMT_FELICA) ? supported_212 : supported_106;
  return NFC_SUCCESS;
}

static int
fake_select_passive_target(nfc_device *pnd, const nfc_modulation nm,
                           const uint8_t *pbtInitData, const size_t szInitData,
                           nfc_target *pnt)
{
  const size_t index = state.passive_call_count++;
  (void)pnd;
  (void)nm;

  if (index < MAX_PASSIVE_CALLS) {
    size_t copy_len = szInitData;

    if (copy_len > MAX_PAYLOAD_LEN) {
      copy_len = MAX_PAYLOAD_LEN;
    }

    state.passive_payload_lens[index] = szInitData;
    if (pbtInitData && copy_len > 0) {
      memcpy(state.passive_payloads[index], pbtInitData, copy_len);
    }
  }

  if (index >= MAX_PASSIVE_CALLS) {
    return 0;
  }

  if (state.passive_results[index] > 0 && pnt) {
    memcpy(pnt, &state.passive_targets[index], sizeof(*pnt));
  }

  return state.passive_results[index];
}

static int
fake_poll_target(nfc_device *pnd, const nfc_modulation *pnmModulations,
                 const size_t szModulations, const uint8_t uiPollNr,
                 const uint8_t btPeriod, nfc_target *pnt)
{
  (void)pnd;
  (void)pnmModulations;
  (void)szModulations;
  (void)uiPollNr;
  (void)btPeriod;
  (void)pnt;
  state.poll_target_calls++;
  return state.poll_target_return;
}

static int
fake_select_dep_target(nfc_device *pnd, const nfc_dep_mode ndm,
                       const nfc_baud_rate nbr, const nfc_dep_info *pndiInitiator,
                       nfc_target *pnt, const int timeout)
{
  const size_t index = state.dep_call_count++;
  (void)pnd;
  (void)ndm;
  (void)nbr;
  (void)pndiInitiator;

  if (index < MAX_DEP_CALLS) {
    state.dep_timeouts[index] = timeout;
  }

  if (index >= MAX_DEP_CALLS) {
    return 0;
  }

  if (state.dep_results[index] > 0 && pnt) {
    *pnt = make_target((uint8_t)(0x40 + index));
  }

  return state.dep_results[index];
}

static int
fake_deselect_target(nfc_device *pnd)
{
  (void)pnd;
  state.deselect_calls++;
  return NFC_SUCCESS;
}

static int
fake_target_is_present(nfc_device *pnd, const nfc_target *pnt)
{
  (void)pnd;
  (void)pnt;
  state.target_is_present_calls++;
  return state.target_is_present_return;
}

static const struct nfc_driver TEST_DRIVER = {
  .name = TEST_DRIVER_NAME,
  .scan_type = NOT_INTRUSIVE,
  .initiator_init = fake_initiator_init,
  .initiator_init_secure_element = fake_initiator_init_secure_element,
  .initiator_select_passive_target = fake_select_passive_target,
  .initiator_poll_target = fake_poll_target,
  .initiator_select_dep_target = fake_select_dep_target,
  .initiator_deselect_target = fake_deselect_target,
  .initiator_target_is_present = fake_target_is_present,
  .device_set_property_bool = fake_set_property_bool,
  .device_set_property_int = fake_set_property_int,
  .get_supported_modulation = fake_get_supported_modulation,
  .get_supported_baud_rate = fake_get_supported_baud_rate,
};

int
main(void)
{
  nfc_device *device = make_device();
  nfc_modulation nm_iso14443a = {
    .nmt = NMT_ISO14443A,
    .nbr = NBR_106,
  };
  nfc_modulation poll_modulations[] = {
    {
      .nmt = NMT_ISO14443A,
      .nbr = NBR_106,
    },
  };
  nfc_target target;
  nfc_target targets[2];
  static const uint8_t uid7[] = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07};
  static const uint8_t expected_cascade[] = {0x88, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07};
  static const property_bool_call expected_init_sequence[] = {
    {NP_ACTIVATE_FIELD, false},
    {NP_ACTIVATE_FIELD, true},
    {NP_INFINITE_SELECT, true},
    {NP_AUTO_ISO14443_4, true},
    {NP_FORCE_ISO14443_A, true},
    {NP_FORCE_SPEED_106, true},
    {NP_ACCEPT_INVALID_FRAMES, false},
    {NP_ACCEPT_MULTIPLE_FRAMES, false},
  };
  int res;
  size_t i;

  CHECK(device != NULL, "nfc_device_new() should create a fake device");

  reset_fake_state();
  res = nfc_device_set_property_int(device, NP_TIMEOUT_COMMAND, 42);
  CHECK(res == NFC_SUCCESS, "nfc_device_set_property_int() should dispatch to fake driver");
  CHECK(state.property_int_call_count == 1, "property_int should be called exactly once");
  CHECK(state.property_int_calls[0].property == NP_TIMEOUT_COMMAND,
        "property_int should preserve the property enum");
  CHECK(state.property_int_calls[0].value == 42,
        "property_int should preserve the integer value");

  reset_fake_state();
  res = nfc_device_set_property_bool(device, NP_INFINITE_SELECT, true);
  CHECK(res == NFC_SUCCESS, "nfc_device_set_property_bool() should dispatch to fake driver");
  CHECK(state.property_bool_call_count == 1, "property_bool should be called exactly once");
  CHECK(state.property_bool_calls[0].property == NP_INFINITE_SELECT,
        "property_bool should preserve the property enum");
  CHECK(state.property_bool_calls[0].value == true,
        "property_bool should preserve the boolean value");
  CHECK(device->bInfiniteSelect == true,
        "fake property handler should update the device infinite-select flag");

  reset_fake_state();
  res = nfc_initiator_init(device);
  CHECK(res == 77, "nfc_initiator_init() should return the driver result");
  CHECK(state.initiator_init_calls == 1, "initiator_init should be called exactly once");
  CHECK(state.property_bool_call_count == sizeof(expected_init_sequence) / sizeof(expected_init_sequence[0]),
        "initiator_init should apply the full property sequence");
  for (i = 0; i < sizeof(expected_init_sequence) / sizeof(expected_init_sequence[0]); i++) {
    CHECK(state.property_bool_calls[i].property == expected_init_sequence[i].property,
          "initiator_init property #%lu should match C behavior", (unsigned long)i);
    CHECK(state.property_bool_calls[i].value == expected_init_sequence[i].value,
          "initiator_init property value #%lu should match C behavior", (unsigned long)i);
  }

  reset_fake_state();
  res = nfc_initiator_init_secure_element(device);
  CHECK(res == 88,
        "nfc_initiator_init_secure_element() should return the driver result");
  CHECK(state.initiator_init_secure_element_calls == 1,
        "initiator_init_secure_element should be called exactly once");

  reset_fake_state();
  state.passive_results[0] = 1;
  state.passive_targets[0] = make_target(0x11);
  memset(&target, 0, sizeof(target));
  res = nfc_initiator_select_passive_target(device, nm_iso14443a,
                                            uid7, sizeof(uid7), &target);
  CHECK(res == 1, "nfc_initiator_select_passive_target() should return the fake target");
  CHECK(state.passive_call_count == 1, "select_passive_target should be called once");
  CHECK(state.passive_payload_lens[0] == sizeof(expected_cascade),
        "ISO14443A init payload should be cascade-expanded");
  CHECK(memcmp(state.passive_payloads[0], expected_cascade, sizeof(expected_cascade)) == 0,
        "ISO14443A init payload should match the cascade UID helper");

  reset_fake_state();
  device->bInfiniteSelect = true;
  state.passive_results[0] = 1;
  state.passive_results[1] = 1;
  state.passive_targets[0] = make_target(0x22);
  state.passive_targets[1] = make_target(0x22);
  memset(targets, 0, sizeof(targets));
  res = nfc_initiator_list_passive_targets(device, nm_iso14443a, targets, 2);
  CHECK(res == 1, "list_passive_targets should stop when a duplicate target is seen");
  CHECK(state.passive_call_count == 2,
        "list_passive_targets should stop after the duplicate response");
  CHECK(state.deselect_calls == 1,
        "list_passive_targets should deselect once before encountering the duplicate");
  CHECK(state.property_bool_call_count == 2,
        "list_passive_targets should toggle NP_INFINITE_SELECT off then on");
  CHECK(state.property_bool_calls[0].property == NP_INFINITE_SELECT &&
        state.property_bool_calls[0].value == false,
        "list_passive_targets should first disable infinite select");
  CHECK(state.property_bool_calls[1].property == NP_INFINITE_SELECT &&
        state.property_bool_calls[1].value == true,
        "list_passive_targets should restore infinite select");
  CHECK(device->bInfiniteSelect == true,
        "list_passive_targets should restore the original infinite-select state");
  CHECK(memcmp(&targets[0], &state.passive_targets[0], sizeof(nfc_target)) == 0,
        "list_passive_targets should copy the first discovered target");

  reset_fake_state();
  state.poll_target_return = 5;
  memset(&target, 0, sizeof(target));
  res = nfc_initiator_poll_target(device, poll_modulations,
                                  sizeof(poll_modulations) / sizeof(poll_modulations[0]),
                                  2, 1, &target);
  CHECK(res == 5, "nfc_initiator_poll_target() should return the fake driver result");
  CHECK(state.poll_target_calls == 1, "poll_target should be called exactly once");

  reset_fake_state();
  state.dep_results[0] = 4;
  memset(&target, 0, sizeof(target));
  res = nfc_initiator_select_dep_target(device, NDM_PASSIVE, NBR_106,
                                        NULL, &target, 123);
  CHECK(res == 4,
        "nfc_initiator_select_dep_target() should return the fake driver result");
  CHECK(state.dep_call_count == 1, "select_dep_target should be called exactly once");
  CHECK(state.dep_timeouts[0] == 123,
        "select_dep_target should preserve the caller timeout");

  reset_fake_state();
  device->bInfiniteSelect = false;
  state.dep_results[0] = NFC_ETIMEOUT;
  state.dep_results[1] = NFC_ETIMEOUT;
  state.dep_results[2] = 1;
  memset(&target, 0, sizeof(target));
  res = nfc_initiator_poll_dep_target(device, NDM_PASSIVE, NBR_106,
                                      NULL, &target, 1000);
  CHECK(res == 1, "poll_dep_target should keep retrying until a target is found");
  CHECK(state.dep_call_count == 3, "poll_dep_target should perform three retries");
  CHECK(state.dep_timeouts[0] == 300 && state.dep_timeouts[1] == 300 &&
        state.dep_timeouts[2] == 300,
        "poll_dep_target should use the fixed 300ms retry period");
  CHECK(state.property_bool_call_count == 2,
        "poll_dep_target should toggle NP_INFINITE_SELECT on and then restore it");
  CHECK(state.property_bool_calls[0].property == NP_INFINITE_SELECT &&
        state.property_bool_calls[0].value == true,
        "poll_dep_target should enable infinite select before polling");
  CHECK(state.property_bool_calls[1].property == NP_INFINITE_SELECT &&
        state.property_bool_calls[1].value == false,
        "poll_dep_target should restore infinite select after polling");
  CHECK(device->bInfiniteSelect == false,
        "poll_dep_target should restore the original infinite-select state");

  reset_fake_state();
  res = nfc_initiator_deselect_target(device);
  CHECK(res == NFC_SUCCESS,
        "nfc_initiator_deselect_target() should dispatch to the fake driver");
  CHECK(state.deselect_calls == 1, "deselect_target should be called exactly once");

  reset_fake_state();
  state.target_is_present_return = 1;
  res = nfc_initiator_target_is_present(device, &target);
  CHECK(res == 1,
        "nfc_initiator_target_is_present() should return the fake driver result");
  CHECK(state.target_is_present_calls == 1,
        "target_is_present should be called exactly once");

  destroy_device(device);
  return 0;
}
