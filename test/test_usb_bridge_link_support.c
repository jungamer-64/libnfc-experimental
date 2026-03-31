#include <stddef.h>
#include <stdint.h>

#include <nfc/nfc.h>

#include "nfc-internal.h"

/*
 * Build-tree link support for test_usbbus_helpers.
 * The Rust static library references internal bridge symbols that are not part
 * of the exported shared-library ABI, but this test only exercises usb_*.
 */

void
nfc_rs_context_log_init(const nfc_context *context)
{
  (void)context;
}

void
nfc_rs_context_log_exit(void)
{
}

void
nfc_rs_log_message(uint8_t group, const char *category, uint8_t priority,
                   const char *message)
{
  (void)group;
  (void)category;
  (void)priority;
  (void)message;
}

void
snprint_nfc_target(char *dst, size_t size, const nfc_target *pnt, bool verbose)
{
  (void)pnt;
  (void)verbose;
  if (dst != NULL && size > 0)
    dst[0] = '\0';
}

void
iso14443_cascade_uid(const uint8_t *abtUID, const size_t szUID,
                     uint8_t *pbtCascadedUID, size_t *pszCascadedUID)
{
  (void)abtUID;
  (void)szUID;
  (void)pbtCascadedUID;
  if (pszCascadedUID != NULL)
    *pszCascadedUID = 0;
}

const struct nfc_driver acr122_pcsc_driver = {0};
const struct nfc_driver acr122_usb_driver = {0};
const struct nfc_driver acr122s_driver = {0};
const struct nfc_driver arygon_driver = {0};
const struct nfc_driver pcsc_driver = {0};
const struct nfc_driver pn53x_usb_driver = {0};
const struct nfc_driver pn532_i2c_driver = {0};
const struct nfc_driver pn532_spi_driver = {0};
const struct nfc_driver pn532_uart_driver = {0};
const struct nfc_driver pn71xx_driver = {0};
