#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <nfc/nfc.h>

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
  int target_text_len;
  const char *version;
  const char *baud_label;
  const char *modulation_label;

  nfc_init(&context);

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
    return 5;
  }
  if (strstr(target_text, "ISO/IEC 14443A") == NULL ||
      strstr(target_text, "106 kbps") == NULL) {
    fprintf(stderr, "str_nfc_target() returned unexpected text: %s\n", target_text);
    nfc_free(target_text);
    if (context) {
      nfc_exit(context);
    }
    return 6;
  }
  nfc_free(target_text);

  target_text_len = str_nfc_target(&empty_target_text, NULL, false);
  if (target_text_len != 0 || empty_target_text == NULL || empty_target_text[0] != '\0') {
    fprintf(stderr, "str_nfc_target(NULL) returned unexpected data\n");
    nfc_free(empty_target_text);
    if (context) {
      nfc_exit(context);
    }
    return 7;
  }
  nfc_free(empty_target_text);

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
    return 8;
  }
  if (strstr(target_text, "D.E.P. (106 kbpsactive mode) target:") == NULL ||
      strstr(target_text, "NFCID3: 01  02  03  04  05  06  07  08  09  0a") == NULL) {
    fprintf(stderr, "str_nfc_target() returned unexpected DEP text: %s\n", target_text);
    nfc_free(target_text);
    if (context) {
      nfc_exit(context);
    }
    return 9;
  }
  nfc_free(target_text);

  if (context) {
    nfc_exit(context);
  }

  return 0;
}
