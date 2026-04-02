#include <stddef.h>
#include <stdint.h>

#include "libnfc_rs_private.h"

/*
 * Build-tree link support for test_usbbus_helpers.
 * The Rust static library now owns several builtin drivers and bus helpers, so
 * this target provides the internal bridge symbols that are intentionally not
 * part of the exported shared-library ABI while the test only exercises usb_*.
 */

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

const uint8_t pn53x_ack_frame[PN53x_ACK_FRAME__LEN] = {0x00, 0x00, 0xff,
                                                       0x00, 0xff, 0x00};
const uint8_t pn53x_nack_frame[PN53x_ACK_FRAME__LEN] = {0x00, 0x00, 0xff,
                                                        0xff, 0x00, 0x00};

static int
stub_pn53x_error(void)
{
  return NFC_ENOTIMPL;
}

int
pn53x_init(struct nfc_device *pnd)
{
  (void)pnd;
  return stub_pn53x_error();
}

int
pn53x_check_communication(struct nfc_device *pnd)
{
  (void)pnd;
  return stub_pn53x_error();
}

#ifndef PROXIMATE_C_FFI
int
pn53x_transceive(struct nfc_device *pnd, const uint8_t *pbtTx, const size_t szTx,
                 uint8_t *pbtRx, const size_t szRxLen, int timeout)
{
  (void)pnd;
  (void)pbtTx;
  (void)szTx;
  (void)pbtRx;
  (void)szRxLen;
  (void)timeout;
  return stub_pn53x_error();
}

int
pn53x_write_register(struct nfc_device *pnd, uint16_t ui16Reg,
                     uint8_t ui8SymbolMask, uint8_t ui8Value)
{
  (void)pnd;
  (void)ui16Reg;
  (void)ui8SymbolMask;
  (void)ui8Value;
  return stub_pn53x_error();
}
#endif

int
pn53x_set_property_int(struct nfc_device *pnd, const nfc_property property,
                       const int value)
{
  (void)pnd;
  (void)property;
  (void)value;
  return stub_pn53x_error();
}

int
pn53x_set_property_bool(struct nfc_device *pnd, const nfc_property property,
                        const bool bEnable)
{
  (void)pnd;
  (void)property;
  (void)bEnable;
  return stub_pn53x_error();
}

int
pn53x_idle(struct nfc_device *pnd)
{
  (void)pnd;
  return stub_pn53x_error();
}

int
pn53x_initiator_init(struct nfc_device *pnd)
{
  (void)pnd;
  return stub_pn53x_error();
}

int
pn532_initiator_init_secure_element(struct nfc_device *pnd)
{
  (void)pnd;
  return stub_pn53x_error();
}

int
pn53x_initiator_select_passive_target(struct nfc_device *pnd,
                                      const nfc_modulation nm,
                                      const uint8_t *pbtInitData,
                                      const size_t szInitData, nfc_target *pnt)
{
  (void)pnd;
  (void)nm;
  (void)pbtInitData;
  (void)szInitData;
  (void)pnt;
  return stub_pn53x_error();
}

int
pn53x_initiator_poll_target(struct nfc_device *pnd,
                            const nfc_modulation *pnmModulations,
                            const size_t szModulations,
                            const uint8_t uiPollNr,
                            const uint8_t uiPeriod, nfc_target *pnt)
{
  (void)pnd;
  (void)pnmModulations;
  (void)szModulations;
  (void)uiPollNr;
  (void)uiPeriod;
  (void)pnt;
  return stub_pn53x_error();
}

int
pn53x_initiator_select_dep_target(struct nfc_device *pnd,
                                  const nfc_dep_mode ndm,
                                  const nfc_baud_rate nbr,
                                  const nfc_dep_info *pndiInitiator,
                                  nfc_target *pnt, const int timeout)
{
  (void)pnd;
  (void)ndm;
  (void)nbr;
  (void)pndiInitiator;
  (void)pnt;
  (void)timeout;
  return stub_pn53x_error();
}

int
pn53x_initiator_transceive_bits(struct nfc_device *pnd,
                                const uint8_t *pbtTx, const size_t szTxBits,
                                const uint8_t *pbtTxPar, uint8_t *pbtRx,
                                uint8_t *pbtRxPar)
{
  (void)pnd;
  (void)pbtTx;
  (void)szTxBits;
  (void)pbtTxPar;
  (void)pbtRx;
  (void)pbtRxPar;
  return stub_pn53x_error();
}

int
pn53x_initiator_transceive_bytes(struct nfc_device *pnd,
                                 const uint8_t *pbtTx, const size_t szTx,
                                 uint8_t *pbtRx, const size_t szRx,
                                 int timeout)
{
  (void)pnd;
  (void)pbtTx;
  (void)szTx;
  (void)pbtRx;
  (void)szRx;
  (void)timeout;
  return stub_pn53x_error();
}

int
pn53x_initiator_transceive_bits_timed(struct nfc_device *pnd,
                                      const uint8_t *pbtTx,
                                      const size_t szTxBits,
                                      const uint8_t *pbtTxPar, uint8_t *pbtRx,
                                      uint8_t *pbtRxPar, uint32_t *cycles)
{
  (void)pnd;
  (void)pbtTx;
  (void)szTxBits;
  (void)pbtTxPar;
  (void)pbtRx;
  (void)pbtRxPar;
  (void)cycles;
  return stub_pn53x_error();
}

int
pn53x_initiator_transceive_bytes_timed(struct nfc_device *pnd,
                                       const uint8_t *pbtTx,
                                       const size_t szTx, uint8_t *pbtRx,
                                       const size_t szRx, uint32_t *cycles)
{
  (void)pnd;
  (void)pbtTx;
  (void)szTx;
  (void)pbtRx;
  (void)szRx;
  (void)cycles;
  return stub_pn53x_error();
}

int
pn53x_initiator_deselect_target(struct nfc_device *pnd)
{
  (void)pnd;
  return stub_pn53x_error();
}

int
pn53x_initiator_target_is_present(struct nfc_device *pnd, const nfc_target *pnt)
{
  (void)pnd;
  (void)pnt;
  return stub_pn53x_error();
}

int
pn53x_target_init(struct nfc_device *pnd, nfc_target *pnt, uint8_t *pbtRx,
                  const size_t szRxLen, int timeout)
{
  (void)pnd;
  (void)pnt;
  (void)pbtRx;
  (void)szRxLen;
  (void)timeout;
  return stub_pn53x_error();
}

int
pn53x_target_receive_bits(struct nfc_device *pnd, uint8_t *pbtRx,
                          const size_t szRxLen, uint8_t *pbtRxPar)
{
  (void)pnd;
  (void)pbtRx;
  (void)szRxLen;
  (void)pbtRxPar;
  return stub_pn53x_error();
}

int
pn53x_target_receive_bytes(struct nfc_device *pnd, uint8_t *pbtRx,
                           const size_t szRxLen, int timeout)
{
  (void)pnd;
  (void)pbtRx;
  (void)szRxLen;
  (void)timeout;
  return stub_pn53x_error();
}

int
pn53x_target_send_bits(struct nfc_device *pnd, const uint8_t *pbtTx,
                       const size_t szTxBits, const uint8_t *pbtTxPar)
{
  (void)pnd;
  (void)pbtTx;
  (void)szTxBits;
  (void)pbtTxPar;
  return stub_pn53x_error();
}

int
pn53x_target_send_bytes(struct nfc_device *pnd, const uint8_t *pbtTx,
                        const size_t szTx, int timeout)
{
  (void)pnd;
  (void)pbtTx;
  (void)szTx;
  (void)timeout;
  return stub_pn53x_error();
}

const char *
pn53x_strerror(const struct nfc_device *pnd)
{
  (void)pnd;
  return "pn53x stubs unavailable in test_usbbus_helpers";
}

int
pn53x_PowerDown(struct nfc_device *pnd)
{
  (void)pnd;
  return stub_pn53x_error();
}

#ifndef PROXIMATE_C_FFI
int
pn532_SAMConfiguration(struct nfc_device *pnd, const pn532_sam_mode mode,
                       int timeout)
{
  (void)pnd;
  (void)mode;
  (void)timeout;
  return stub_pn53x_error();
}
#endif

int
pn53x_check_ack_frame(struct nfc_device *pnd, const uint8_t *pbtRxFrame,
                      const size_t szRxFrameLen)
{
  (void)pnd;
  (void)pbtRxFrame;
  (void)szRxFrameLen;
  return stub_pn53x_error();
}

int
pn53x_build_frame(uint8_t *pbtFrame, size_t *pszFrame,
                  const uint8_t *pbtData, const size_t szData)
{
  (void)pbtFrame;
  (void)pszFrame;
  (void)pbtData;
  (void)szData;
  return stub_pn53x_error();
}

int
pn53x_get_supported_modulation(nfc_device *pnd, const nfc_mode mode,
                               const nfc_modulation_type **const supported_mt)
{
  (void)pnd;
  (void)mode;
  if (supported_mt != NULL)
    *supported_mt = NULL;
  return stub_pn53x_error();
}

int
pn53x_get_supported_baud_rate(nfc_device *pnd, const nfc_mode mode,
                              const nfc_modulation_type nmt,
                              const nfc_baud_rate **const supported_br)
{
  (void)pnd;
  (void)mode;
  (void)nmt;
  if (supported_br != NULL)
    *supported_br = NULL;
  return stub_pn53x_error();
}

int
pn53x_get_information_about(nfc_device *pnd, char **pbuf)
{
  (void)pnd;
  if (pbuf != NULL)
    *pbuf = NULL;
  return stub_pn53x_error();
}

void *
pn53x_data_new(struct nfc_device *pnd, const struct pn53x_io *io)
{
  (void)pnd;
  (void)io;
  return NULL;
}

void
pn53x_data_free(struct nfc_device *pnd)
{
  (void)pnd;
}
