#include <stdbool.h>
#include <stdint.h>
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

#include <nfc/nfc.h>

#include "libnfc/nfc-internal.h"

#define TEST_DRIVER_NAME "rust_core_device_io_test"
#define TEST_DEVICE_NAME "rust-core-device"
#define MAX_PROPERTY_CALLS 16
#define MAX_BAUD_CALLS 4

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
  nfc_mode mode;
  nfc_modulation_type modulation_type;
} baud_rate_call;

typedef struct {
  size_t property_bool_call_count;
  property_bool_call property_bool_calls[MAX_PROPERTY_CALLS];
  size_t target_init_call_count;
  size_t target_init_rx_len;
  int target_init_timeout;
  size_t transceive_bytes_call_count;
  size_t transceive_bytes_tx_len;
  size_t transceive_bytes_rx_len;
  int transceive_bytes_timeout;
  size_t transceive_bits_call_count;
  size_t transceive_bits_tx_bits_len;
  size_t transceive_bytes_timed_call_count;
  size_t transceive_bytes_timed_tx_len;
  size_t transceive_bytes_timed_rx_len;
  size_t transceive_bits_timed_call_count;
  size_t transceive_bits_timed_tx_bits_len;
  size_t target_send_bytes_call_count;
  size_t target_send_bytes_len;
  int target_send_bytes_timeout;
  size_t target_receive_bytes_call_count;
  size_t target_receive_bytes_len;
  int target_receive_bytes_timeout;
  size_t target_send_bits_call_count;
  size_t target_send_bits_len;
  size_t target_receive_bits_call_count;
  size_t target_receive_bits_len;
  size_t get_supported_modulation_call_count;
  nfc_mode supported_modulation_mode;
  size_t get_supported_baud_rate_call_count;
  baud_rate_call supported_baud_rate_calls[MAX_BAUD_CALLS];
  size_t information_about_call_count;
  size_t abort_call_count;
  size_t idle_call_count;
} fake_state;

static fake_state state;
static const struct nfc_driver TEST_DRIVER;

static void
reset_fake_state(void)
{
  memset(&state, 0, sizeof(state));
}

static nfc_device *
make_device(void)
{
  nfc_device *device = calloc(1, sizeof(*device));

  if (!device) {
    return NULL;
  }

  snprintf(device->connstring, sizeof(device->connstring), "%s", TEST_DRIVER_NAME);
  snprintf(device->name, sizeof(device->name), "%s", TEST_DEVICE_NAME);
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
fake_target_init(nfc_device *pnd, nfc_target *pnt, uint8_t *pbtRx,
                 const size_t szRx, int timeout)
{
  (void)pnd;
  state.target_init_call_count++;
  state.target_init_rx_len = szRx;
  state.target_init_timeout = timeout;

  if (pnt) {
    memset(pnt, 0, sizeof(*pnt));
    pnt->nm.nmt = NMT_ISO14443A;
    pnt->nm.nbr = NBR_106;
    pnt->nti.nai.szUidLen = 1;
    pnt->nti.nai.abtUid[0] = 0x22;
  }
  if (pbtRx && szRx > 0) {
    pbtRx[0] = 0x44;
  }
  return 31;
}

static int
fake_initiator_transceive_bytes(nfc_device *pnd, const uint8_t *pbtTx,
                                const size_t szTx, uint8_t *pbtRx,
                                const size_t szRx, int timeout)
{
  (void)pnd;
  (void)pbtTx;
  state.transceive_bytes_call_count++;
  state.transceive_bytes_tx_len = szTx;
  state.transceive_bytes_rx_len = szRx;
  state.transceive_bytes_timeout = timeout;

  if (pbtRx && szRx > 0) {
    pbtRx[0] = 0x51;
  }
  return 32;
}

static int
fake_initiator_transceive_bits(nfc_device *pnd, const uint8_t *pbtTx,
                               const size_t szTxBits, const uint8_t *pbtTxPar,
                               uint8_t *pbtRx, uint8_t *pbtRxPar)
{
  (void)pnd;
  (void)pbtTx;
  (void)pbtTxPar;
  state.transceive_bits_call_count++;
  state.transceive_bits_tx_bits_len = szTxBits;

  if (pbtRx) {
    pbtRx[0] = 0x61;
  }
  if (pbtRxPar) {
    pbtRxPar[0] = 0x01;
  }
  return 33;
}

static int
fake_initiator_transceive_bytes_timed(nfc_device *pnd, const uint8_t *pbtTx,
                                      const size_t szTx, uint8_t *pbtRx,
                                      const size_t szRx, uint32_t *cycles)
{
  (void)pnd;
  (void)pbtTx;
  state.transceive_bytes_timed_call_count++;
  state.transceive_bytes_timed_tx_len = szTx;
  state.transceive_bytes_timed_rx_len = szRx;

  if (pbtRx && szRx > 0) {
    pbtRx[0] = 0x71;
  }
  if (cycles) {
    *cycles = 1234;
  }
  return 34;
}

static int
fake_initiator_transceive_bits_timed(nfc_device *pnd, const uint8_t *pbtTx,
                                     const size_t szTxBits,
                                     const uint8_t *pbtTxPar, uint8_t *pbtRx,
                                     uint8_t *pbtRxPar, uint32_t *cycles)
{
  (void)pnd;
  (void)pbtTx;
  (void)pbtTxPar;
  state.transceive_bits_timed_call_count++;
  state.transceive_bits_timed_tx_bits_len = szTxBits;

  if (pbtRx) {
    pbtRx[0] = 0x81;
  }
  if (pbtRxPar) {
    pbtRxPar[0] = 0x02;
  }
  if (cycles) {
    *cycles = 5678;
  }
  return 35;
}

static int
fake_target_send_bytes(nfc_device *pnd, const uint8_t *pbtTx,
                       const size_t szTx, int timeout)
{
  (void)pnd;
  (void)pbtTx;
  state.target_send_bytes_call_count++;
  state.target_send_bytes_len = szTx;
  state.target_send_bytes_timeout = timeout;
  return 36;
}

static int
fake_target_receive_bytes(nfc_device *pnd, uint8_t *pbtRx,
                          const size_t szRx, int timeout)
{
  (void)pnd;
  state.target_receive_bytes_call_count++;
  state.target_receive_bytes_len = szRx;
  state.target_receive_bytes_timeout = timeout;

  if (pbtRx && szRx > 0) {
    pbtRx[0] = 0x91;
  }
  return 37;
}

static int
fake_target_send_bits(nfc_device *pnd, const uint8_t *pbtTx,
                      const size_t szTxBits, const uint8_t *pbtTxPar)
{
  (void)pnd;
  (void)pbtTx;
  (void)pbtTxPar;
  state.target_send_bits_call_count++;
  state.target_send_bits_len = szTxBits;
  return 38;
}

static int
fake_target_receive_bits(nfc_device *pnd, uint8_t *pbtRx,
                         const size_t szRx, uint8_t *pbtRxPar)
{
  (void)pnd;
  state.target_receive_bits_call_count++;
  state.target_receive_bits_len = szRx;

  if (pbtRx && szRx > 0) {
    pbtRx[0] = 0xa1;
  }
  if (pbtRxPar) {
    pbtRxPar[0] = 0x03;
  }
  return 39;
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
  state.get_supported_modulation_call_count++;
  state.supported_modulation_mode = mode;
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
  if (state.get_supported_baud_rate_call_count < MAX_BAUD_CALLS) {
    state.supported_baud_rate_calls[state.get_supported_baud_rate_call_count].mode = mode;
    state.supported_baud_rate_calls[state.get_supported_baud_rate_call_count].modulation_type = nmt;
  }
  state.get_supported_baud_rate_call_count++;
  *supported_br = (nmt == NMT_FELICA) ? supported_212 : supported_106;
  return NFC_SUCCESS;
}

static int
fake_device_get_information_about(nfc_device *pnd, char **buf)
{
  static const char info[] = "driver-info";
  char *copy;

  (void)pnd;
  state.information_about_call_count++;
  if (buf) {
    copy = malloc(sizeof(info));
    if (!copy) {
      *buf = NULL;
      return -1;
    }
    memcpy(copy, info, sizeof(info));
    *buf = copy;
  }
  return 40;
}

static int
fake_abort_command(nfc_device *pnd)
{
  (void)pnd;
  state.abort_call_count++;
  return 41;
}

static int
fake_idle(nfc_device *pnd)
{
  (void)pnd;
  state.idle_call_count++;
  return 42;
}

static const struct nfc_driver TEST_DRIVER = {
  .name = TEST_DRIVER_NAME,
  .scan_type = NOT_INTRUSIVE,
  .scan = NULL,
  .open = NULL,
  .close = NULL,
  .strerror = NULL,
  .initiator_init = NULL,
  .initiator_init_secure_element = NULL,
  .initiator_select_passive_target = NULL,
  .initiator_poll_target = NULL,
  .initiator_select_dep_target = NULL,
  .initiator_deselect_target = NULL,
  .initiator_transceive_bytes = fake_initiator_transceive_bytes,
  .initiator_transceive_bits = fake_initiator_transceive_bits,
  .initiator_transceive_bytes_timed = fake_initiator_transceive_bytes_timed,
  .initiator_transceive_bits_timed = fake_initiator_transceive_bits_timed,
  .initiator_target_is_present = NULL,
  .target_init = fake_target_init,
  .target_send_bytes = fake_target_send_bytes,
  .target_receive_bytes = fake_target_receive_bytes,
  .target_send_bits = fake_target_send_bits,
  .target_receive_bits = fake_target_receive_bits,
  .device_set_property_bool = fake_set_property_bool,
  .device_set_property_int = NULL,
  .get_supported_modulation = fake_get_supported_modulation,
  .get_supported_baud_rate = fake_get_supported_baud_rate,
  .device_get_information_about = fake_device_get_information_about,
  .abort_command = fake_abort_command,
  .idle = fake_idle,
  .powerdown = NULL,
};

static int
capture_perror_output(nfc_device *device, const char *label,
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

  nfc_perror(device, label);
  fflush(stderr);

  if (dup2(saved_stderr, fileno(stderr)) < 0) {
    close(saved_stderr);
    fclose(tmp);
    return 0;
  }
  close(saved_stderr);

  rewind(tmp);
  read_len = fread(buffer, 1, buffer_size - 1, tmp);
  buffer[read_len] = '\0';
  fclose(tmp);
  return 1;
}

int
main(void)
{
  nfc_device *device = make_device();
  const uint8_t tx[] = {0xaa, 0x55};
  const uint8_t tx_parity[] = {0x01};
  uint8_t rx[4] = {0};
  uint8_t rx_bits[1] = {0};
  uint8_t rx_parity[1] = {0};
  uint32_t cycles = 0;
  nfc_target target;
  const nfc_modulation_type *supported_mt = NULL;
  const nfc_baud_rate *supported_br = NULL;
  char *info = NULL;
  char strerror_buf[8];
  char perror_output[128];

  CHECK(device != NULL, "device allocation should succeed");
  reset_fake_state();
  memset(&target, 0, sizeof(target));

  CHECK(nfc_target_init(device, &target, rx, sizeof(rx), 250) == 31,
        "nfc_target_init() should dispatch to the fake driver");
  CHECK(rx[0] == 0x44, "target init should be able to populate rx bytes");
  CHECK(state.target_init_call_count == 1, "target init should run once");
  CHECK(state.target_init_rx_len == sizeof(rx), "target init should observe rx len");
  CHECK(state.target_init_timeout == 250, "target init should observe timeout");
  CHECK(state.property_bool_call_count == 8,
        "target init should apply the full property sequence");
  CHECK(state.property_bool_calls[0].property == NP_ACCEPT_INVALID_FRAMES &&
            state.property_bool_calls[0].value == false,
        "target init should start with NP_ACCEPT_INVALID_FRAMES=false");
  CHECK(state.property_bool_calls[7].property == NP_ACTIVATE_FIELD &&
            state.property_bool_calls[7].value == false,
        "target init should end with NP_ACTIVATE_FIELD=false");

  CHECK(nfc_initiator_transceive_bytes(device, tx, sizeof(tx), rx, sizeof(rx), 75) == 32,
        "nfc_initiator_transceive_bytes() should dispatch");
  CHECK(state.transceive_bytes_call_count == 1, "transceive bytes should run once");
  CHECK(state.transceive_bytes_tx_len == sizeof(tx), "transceive bytes should see tx len");
  CHECK(state.transceive_bytes_rx_len == sizeof(rx), "transceive bytes should see rx len");
  CHECK(state.transceive_bytes_timeout == 75, "transceive bytes should see timeout");
  CHECK(rx[0] == 0x51, "transceive bytes should be able to write into rx");

  CHECK(nfc_initiator_transceive_bits(device, tx, 7, tx_parity, rx_bits, 0, rx_parity) == 33,
        "nfc_initiator_transceive_bits() should ignore szRx and dispatch");
  CHECK(state.transceive_bits_call_count == 1, "transceive bits should run once");
  CHECK(state.transceive_bits_tx_bits_len == 7, "transceive bits should see tx bit len");
  CHECK(rx_bits[0] == 0x61, "transceive bits should be able to write into rx");
  CHECK(rx_parity[0] == 0x01, "transceive bits should be able to write parity");

  CHECK(nfc_initiator_transceive_bytes_timed(device, tx, sizeof(tx), rx, sizeof(rx), &cycles) == 34,
        "nfc_initiator_transceive_bytes_timed() should dispatch");
  CHECK(state.transceive_bytes_timed_call_count == 1,
        "transceive bytes timed should run once");
  CHECK(cycles == 1234, "transceive bytes timed should update cycles");

  CHECK(nfc_initiator_transceive_bits_timed(device, tx, 5, tx_parity, rx_bits, 0,
                                            rx_parity, &cycles) == 35,
        "nfc_initiator_transceive_bits_timed() should ignore szRx and dispatch");
  CHECK(state.transceive_bits_timed_call_count == 1,
        "transceive bits timed should run once");
  CHECK(state.transceive_bits_timed_tx_bits_len == 5,
        "transceive bits timed should see tx bit len");
  CHECK(rx_bits[0] == 0x81, "transceive bits timed should update rx");
  CHECK(rx_parity[0] == 0x02, "transceive bits timed should update parity");
  CHECK(cycles == 5678, "transceive bits timed should update cycles");

  CHECK(nfc_target_send_bytes(device, tx, sizeof(tx), 125) == 36,
        "nfc_target_send_bytes() should dispatch");
  CHECK(nfc_target_receive_bytes(device, rx, sizeof(rx), 175) == 37,
        "nfc_target_receive_bytes() should dispatch");
  CHECK(nfc_target_send_bits(device, tx, 9, tx_parity) == 38,
        "nfc_target_send_bits() should dispatch");
  CHECK(nfc_target_receive_bits(device, rx, sizeof(rx), rx_parity) == 39,
        "nfc_target_receive_bits() should dispatch");
  CHECK(state.target_send_bytes_call_count == 1 &&
            state.target_send_bytes_len == sizeof(tx) &&
            state.target_send_bytes_timeout == 125,
        "target send bytes should receive expected arguments");
  CHECK(state.target_receive_bytes_call_count == 1 &&
            state.target_receive_bytes_len == sizeof(rx) &&
            state.target_receive_bytes_timeout == 175,
        "target receive bytes should receive expected arguments");
  CHECK(state.target_send_bits_call_count == 1 && state.target_send_bits_len == 9,
        "target send bits should receive expected arguments");
  CHECK(state.target_receive_bits_call_count == 1 &&
            state.target_receive_bits_len == sizeof(rx),
        "target receive bits should receive expected arguments");
  CHECK(rx[0] == 0xa1, "target receive bits should write rx");
  CHECK(rx_parity[0] == 0x03, "target receive bits should write parity");

  CHECK(nfc_device_get_supported_modulation(device, N_TARGET, &supported_mt) == 0,
        "nfc_device_get_supported_modulation() should dispatch");
  CHECK(nfc_device_get_supported_baud_rate(device, NMT_ISO14443A, &supported_br) == 0,
        "nfc_device_get_supported_baud_rate() should dispatch");
  CHECK(nfc_device_get_supported_baud_rate_target_mode(device, NMT_FELICA, &supported_br) == 0,
        "nfc_device_get_supported_baud_rate_target_mode() should dispatch");
  CHECK(state.get_supported_modulation_call_count == 1 &&
            state.supported_modulation_mode == N_TARGET,
        "supported modulation should preserve requested mode");
  CHECK(state.get_supported_baud_rate_call_count == 2,
        "supported baud rate should be queried twice");
  CHECK(state.supported_baud_rate_calls[0].mode == N_INITIATOR &&
            state.supported_baud_rate_calls[0].modulation_type == NMT_ISO14443A,
        "initiator baud rate query should preserve mode and type");
  CHECK(state.supported_baud_rate_calls[1].mode == N_TARGET &&
            state.supported_baud_rate_calls[1].modulation_type == NMT_FELICA,
        "target-mode baud rate query should preserve mode and type");

  CHECK(nfc_device_get_information_about(device, &info) == 40,
        "nfc_device_get_information_about() should dispatch");
  CHECK(state.information_about_call_count == 1,
        "device information callback should run once");
  CHECK(info != NULL && strcmp(info, "driver-info") == 0,
        "device information should be returned");
  nfc_free(info);
  info = NULL;
  CHECK(nfc_abort_command(device) == 41, "nfc_abort_command() should dispatch");
  CHECK(nfc_idle(device) == 42, "nfc_idle() should dispatch");
  CHECK(state.abort_call_count == 1, "abort callback should run once");
  CHECK(state.idle_call_count == 1, "idle callback should run once");

  CHECK(strcmp(nfc_device_get_name(device), TEST_DEVICE_NAME) == 0,
        "nfc_device_get_name() should expose device name");
  CHECK(strcmp(nfc_device_get_connstring(device), TEST_DRIVER_NAME) == 0,
        "nfc_device_get_connstring() should expose connstring");

  device->last_error = NFC_EDEVNOTSUPP;
  CHECK(nfc_device_get_last_error(device) == NFC_EDEVNOTSUPP,
        "nfc_device_get_last_error() should expose the last error");
  CHECK(strcmp(nfc_strerror(device), "Not Supported by Device") == 0,
        "nfc_strerror() should map known errors");
  CHECK(nfc_strerror_r(device, strerror_buf, sizeof(strerror_buf)) == 0,
        "nfc_strerror_r() should succeed for truncated buffers");
  CHECK(strcmp(strerror_buf, "Not Sup") == 0,
        "nfc_strerror_r() should preserve snprintf-style truncation");
  CHECK(capture_perror_output(device, "core-device-io", perror_output, sizeof(perror_output)),
        "nfc_perror() output should be captured");
  CHECK(strcmp(perror_output, "core-device-io: Not Supported by Device\n") == 0,
        "nfc_perror() should preserve C formatting");

  destroy_device(device);
  return 0;
}
