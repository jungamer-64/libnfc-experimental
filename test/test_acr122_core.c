#include <stdlib.h>
#include <stdint.h>
#include <string.h>

#include "drivers/acr122-core.h"

static void
expect_true(int condition)
{
  if (!condition)
    abort();
}

static void
test_build_direct_transmit(void)
{
  const uint8_t payload[] = {0x4A, 0x01, 0x00};
  const uint8_t expected[] = {
      0xFF, 0x00, 0x00, 0x00, 0x04, 0xD4, 0x4A, 0x01, 0x00,
  };
  uint8_t frame[sizeof(expected)];

  size_t len = acr122_build_direct_transmit_apdu(frame, sizeof(frame), payload,
                                                 sizeof(payload));
  expect_true(len == sizeof(expected));
  expect_true(memcmp(frame, expected, sizeof(expected)) == 0);
}

static void
test_build_get_firmware_version(void)
{
  const uint8_t expected[] = {0xFF, 0x00, 0x48, 0x00, 0x00};
  uint8_t frame[sizeof(expected)];

  size_t len = acr122_build_get_firmware_version_apdu(frame, sizeof(frame));
  expect_true(len == sizeof(expected));
  expect_true(memcmp(frame, expected, sizeof(expected)) == 0);
}

static void
test_build_get_additional_data(void)
{
  const uint8_t expected[] = {0xFF, 0xC0, 0x00, 0x00, 0x08};
  uint8_t frame[sizeof(expected)];

  size_t len =
      acr122_build_get_additional_data_apdu(frame, sizeof(frame), 0x08);
  expect_true(len == sizeof(expected));
  expect_true(memcmp(frame, expected, sizeof(expected)) == 0);
}

static void
test_parse_status_words(void)
{
  struct acr122_status_word status_word;
  const uint8_t more_data[] = {0x61, 0x08};
  const uint8_t app_error[] = {0x63, 0x7F};

  expect_true(acr122_parse_status_words(more_data, sizeof(more_data), &status_word));
  expect_true(status_word.has_more_data);
  expect_true(status_word.more_data_length == 0x08);
  expect_true(!status_word.unexpected);

  expect_true(acr122_parse_status_words(app_error, sizeof(app_error), &status_word));
  expect_true(status_word.application_error);
  expect_true(!status_word.has_more_data);
}

static void
test_matchers(void)
{
  expect_true(acr122_is_usb_device(0x072F, 0x2200));
  expect_true(acr122_is_usb_device(0x072F, 0x90CC));
  expect_true(!acr122_is_usb_device(0x04CC, 0x0531));

  expect_true(acr122_is_pcsc_reader_name("ACS ACR122U PICC Interface 00 00"));
  expect_true(acr122_is_pcsc_reader_name("ACS ACR38U-CCID 00 00"));
  expect_true(!acr122_is_pcsc_reader_name("Feitian R502 CL Reader 0"));
}

static void
test_firmware_prefixes(void)
{
  expect_true(acr122_is_acr122u_firmware("ACR122U203"));
  expect_true(acr122_is_acr122s_firmware("ACR122S101"));
  expect_true(!acr122_is_acr122u_firmware("PN533"));
}

int
main(void)
{
  test_build_direct_transmit();
  test_build_get_firmware_version();
  test_build_get_additional_data();
  test_parse_status_words();
  test_matchers();
  test_firmware_prefixes();
  return 0;
}
