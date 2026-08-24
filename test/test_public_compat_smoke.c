#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

#include <nfc/nfc-emulation.h>
#include <nfc/nfc.h>

static int
exercise_public_entrypoints(nfc_context *context)
{
  static const char unavailable_connstring[] = "public-abi-smoke:no-device";
  uint8_t crc_a_data[6] = {0x01, 0x02, 0x03, 0x04, 0x00, 0x00};
  uint8_t crc_b_data[6] = {0x01, 0x02, 0x03, 0x04, 0x00, 0x00};
  uint8_t crc_a[2] = {0x00, 0x00};
  uint8_t crc_b[2] = {0x00, 0x00};
  uint8_t ats[4] = {0x00, 0x11, 0x22, 0x33};
  uint32_t cycles = 0;
  size_t historical_len = 0;
  nfc_connstring connstrings[1] = {{0}};
  nfc_modulation modulation = {NMT_ISO14443A, NBR_106};
  const nfc_modulation_type *supported_modulations = NULL;
  const nfc_baud_rate *supported_baud_rates = NULL;
  nfc_device *device;
  char *information = NULL;

  iso14443a_crc(crc_a_data, 4, crc_a);
  iso14443a_crc_append(crc_a_data, 4);
  if (memcmp(crc_a, crc_a_data + 4, sizeof(crc_a)) != 0) {
    fprintf(stderr, "iso14443a CRC entrypoints disagree\n");
    return 14;
  }

  iso14443b_crc(crc_b_data, 4, crc_b);
  iso14443b_crc_append(crc_b_data, 4);
  if (memcmp(crc_b, crc_b_data + 4, sizeof(crc_b)) != 0) {
    fprintf(stderr, "iso14443b CRC entrypoints disagree\n");
    return 15;
  }

  if (iso14443a_locate_historical_bytes(ats, sizeof(ats), &historical_len) != ats + 1 ||
      historical_len != 3) {
    fprintf(stderr, "iso14443a historical-byte location failed\n");
    return 16;
  }

  if (nfc_register_driver(NULL) != NFC_EINVARG) {
    fprintf(stderr, "nfc_register_driver() accepted a NULL driver\n");
    return 17;
  }

  (void)nfc_list_devices(context, connstrings, 1);
  device = nfc_open(context, unavailable_connstring);
  if (device != NULL) {
    fprintf(stderr, "nfc_open() accepted an unavailable driver family\n");
    nfc_close(device);
    return 18;
  }
  nfc_close(NULL);

  (void)nfc_abort_command(NULL);
  (void)nfc_idle(NULL);
  (void)nfc_initiator_init(NULL);
  (void)nfc_initiator_init_secure_element(NULL);
  (void)nfc_initiator_select_passive_target(NULL, modulation, NULL, 0, NULL);
  (void)nfc_initiator_list_passive_targets(NULL, modulation, NULL, 0);
  (void)nfc_initiator_poll_target(NULL, &modulation, 1, 1, 1, NULL);
  (void)nfc_initiator_select_dep_target(NULL, NDM_PASSIVE, NBR_106, NULL, NULL, 0);
  (void)nfc_initiator_poll_dep_target(NULL, NDM_PASSIVE, NBR_106, NULL, NULL, 0);
  (void)nfc_initiator_deselect_target(NULL);
  (void)nfc_initiator_target_is_present(NULL, NULL);
  (void)nfc_initiator_transceive_bytes(NULL, NULL, 0, NULL, 0, 0);
  (void)nfc_initiator_transceive_bits(NULL, NULL, 0, NULL, NULL, 0, NULL);
  (void)nfc_initiator_transceive_bytes_timed(NULL, NULL, 0, NULL, 0, &cycles);
  (void)nfc_initiator_transceive_bits_timed(NULL, NULL, 0, NULL, NULL, 0, NULL, &cycles);
  (void)nfc_target_init(NULL, NULL, NULL, 0, 0);
  (void)nfc_target_send_bytes(NULL, NULL, 0, 0);
  (void)nfc_target_receive_bytes(NULL, NULL, 0, 0);
  (void)nfc_target_send_bits(NULL, NULL, 0, NULL);
  (void)nfc_target_receive_bits(NULL, NULL, 0, NULL);

  if (nfc_emulate_target(NULL, NULL, 0) != NFC_EINVARG) {
    fprintf(stderr, "nfc_emulate_target() accepted a NULL emulator\n");
    return 19;
  }

  if (nfc_device_get_name(NULL) != NULL ||
      nfc_device_get_connstring(NULL) != NULL) {
    fprintf(stderr, "device string accessors accepted a NULL device\n");
    return 20;
  }
  (void)nfc_device_get_last_error(NULL);
  (void)nfc_device_get_supported_modulation(NULL, N_INITIATOR, &supported_modulations);
  (void)nfc_device_get_supported_baud_rate(NULL, NMT_ISO14443A, &supported_baud_rates);
  (void)nfc_device_get_supported_baud_rate_target_mode(NULL, NMT_ISO14443A, &supported_baud_rates);
  (void)nfc_device_set_property_bool(NULL, NP_HANDLE_CRC, true);
  (void)nfc_device_set_property_int(NULL, NP_TIMEOUT_COMMAND, 0);
  (void)nfc_device_get_information_about(NULL, &information);
  nfc_free(information);

  if (nfc_strerror(NULL) == NULL) {
    fprintf(stderr, "nfc_strerror() returned NULL\n");
    return 21;
  }
  nfc_perror(NULL, "public-abi-smoke");

  return 0;
}

int
main(void)
{
  nfc_context *context = NULL;
  uint8_t data[4] = {0x01, 0x02, 0x03, 0x04};
  uint8_t crc[2] = {0x00, 0x00};
  static const uint8_t expected_crc[2] = {0x91, 0x39};
  nfc_target target;
  char *target_text = NULL;
  char *empty_target_text = NULL;
  char strerror_buf[8];
  int target_text_len;
  const char *version;
  const char *baud_label;
  const char *modulation_label;
  int entrypoint_status;

  nfc_init(&context);

  entrypoint_status = exercise_public_entrypoints(context);
  if (entrypoint_status != 0) {
    if (context) {
      nfc_exit(context);
    }
    return entrypoint_status;
  }

  iso14443b_crc(data, sizeof(data), crc);
  if (memcmp(crc, expected_crc, sizeof(expected_crc)) != 0) {
    fprintf(stderr, "Unexpected CRC_B bytes: %02x%02x\n", crc[0], crc[1]);
    if (context) {
      nfc_exit(context);
    }
    return 1;
  }

  version = nfc_version();
  if (version == NULL || version[0] == '\0') {
    fprintf(stderr, "nfc_version() returned an empty string\n");
    if (context) {
      nfc_exit(context);
    }
    return 2;
  }

  baud_label = str_nfc_baud_rate(NBR_106);
  if (baud_label == NULL || strcmp(baud_label, "106 kbps") != 0) {
    fprintf(stderr, "str_nfc_baud_rate() returned an unexpected value\n");
    if (context) {
      nfc_exit(context);
    }
    return 3;
  }

  modulation_label = str_nfc_modulation_type(NMT_ISO14443A);
  if (modulation_label == NULL || strcmp(modulation_label, "ISO/IEC 14443A") != 0) {
    fprintf(stderr, "str_nfc_modulation_type() returned an unexpected value\n");
    if (context) {
      nfc_exit(context);
    }
    return 4;
  }

  if (sizeof(nfc_iso14443a_info) != 2 + 1 + sizeof(size_t) + 10 + sizeof(size_t) + 254 ||
      offsetof(nfc_iso14443a_info, btSak) != 2 ||
      offsetof(nfc_iso14443a_info, szUidLen) != 3 ||
      offsetof(nfc_target_info, nai) != 0 ||
      offsetof(nfc_target_info, ndi) != 0 ||
      sizeof(nfc_target_info) < sizeof(nfc_iso14443a_info) ||
      sizeof(nfc_target_info) < sizeof(nfc_dep_info) ||
      offsetof(nfc_target, nm) != sizeof(nfc_target_info)) {
    fprintf(stderr, "Public NFC target ABI layout does not match packed header contract\n");
    if (context) {
      nfc_exit(context);
    }
    return 5;
  }

  memset(&target, 0, sizeof(target));
  target.nm.nmt = NMT_ISO14443A;
  target.nm.nbr = NBR_106;
  target.nti.nai.szUidLen = 4;
  target.nti.nai.abtUid[0] = 0x01;
  target.nti.nai.abtUid[1] = 0x23;
  target.nti.nai.abtUid[2] = 0x45;
  target.nti.nai.abtUid[3] = 0x67;

  target_text_len = str_nfc_target(&target_text, &target, false);
  if (target_text_len <= 0 || target_text == NULL) {
    fprintf(stderr, "str_nfc_target() failed to allocate a rendered target string\n");
    if (context) {
      nfc_exit(context);
    }
    return 6;
  }
  if (strstr(target_text, "ISO/IEC 14443A") == NULL ||
      strstr(target_text, "106 kbps") == NULL) {
    fprintf(stderr, "str_nfc_target() returned unexpected text: %s\n", target_text);
    nfc_free(target_text);
    if (context) {
      nfc_exit(context);
    }
    return 7;
  }
  nfc_free(target_text);

  if (str_nfc_target(NULL, &target, false) != NFC_EINVARG) {
    fprintf(stderr, "str_nfc_target() accepted a NULL output pointer\n");
    if (context) {
      nfc_exit(context);
    }
    return 8;
  }

  target_text_len = str_nfc_target(&empty_target_text, NULL, false);
  if (target_text_len != 0 || empty_target_text == NULL || empty_target_text[0] != '\0') {
    fprintf(stderr, "str_nfc_target(NULL) returned unexpected data\n");
    nfc_free(empty_target_text);
    if (context) {
      nfc_exit(context);
    }
    return 9;
  }
  nfc_free(empty_target_text);
  nfc_free(NULL);

  if (nfc_strerror_r(NULL, NULL, 1) != -1 ||
      nfc_strerror_r(NULL, NULL, 0) != 0) {
    fprintf(stderr, "nfc_strerror_r() did not enforce NULL buffer rules\n");
    if (context) {
      nfc_exit(context);
    }
    return 10;
  }

  memset(strerror_buf, 0xaa, sizeof(strerror_buf));
  if (nfc_strerror_r(NULL, strerror_buf, sizeof(strerror_buf)) != 0 ||
      strcmp(strerror_buf, "Success") != 0) {
    fprintf(stderr, "nfc_strerror_r() did not write a NUL-terminated message\n");
    if (context) {
      nfc_exit(context);
    }
    return 11;
  }

  memset(&target, 0, sizeof(target));
  target.nm.nmt = NMT_DEP;
  target.nm.nbr = NBR_106;
  target.nti.ndi.ndm = NDM_ACTIVE;
  target.nti.ndi.abtNFCID3[0] = 0x01;
  target.nti.ndi.abtNFCID3[1] = 0x02;
  target.nti.ndi.abtNFCID3[2] = 0x03;
  target.nti.ndi.abtNFCID3[3] = 0x04;
  target.nti.ndi.abtNFCID3[4] = 0x05;
  target.nti.ndi.abtNFCID3[5] = 0x06;
  target.nti.ndi.abtNFCID3[6] = 0x07;
  target.nti.ndi.abtNFCID3[7] = 0x08;
  target.nti.ndi.abtNFCID3[8] = 0x09;
  target.nti.ndi.abtNFCID3[9] = 0x0a;

  target_text = NULL;
  target_text_len = str_nfc_target(&target_text, &target, false);
  if (target_text_len <= 0 || target_text == NULL) {
    fprintf(stderr, "str_nfc_target() failed for DEP target\n");
    if (context) {
      nfc_exit(context);
    }
    return 12;
  }
  if (strstr(target_text, "D.E.P. (106 kbpsactive mode) target:") == NULL ||
      strstr(target_text, "NFCID3: 01  02  03  04  05  06  07  08  09  0a") == NULL) {
    fprintf(stderr, "str_nfc_target() returned unexpected DEP text: %s\n", target_text);
    nfc_free(target_text);
    if (context) {
      nfc_exit(context);
    }
    return 13;
  }
  nfc_free(target_text);

  if (context) {
    nfc_exit(context);
  }

  return 0;
}
