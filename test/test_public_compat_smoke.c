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
  const char *version;

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

  if (context) {
    nfc_exit(context);
  }

  return 0;
}
