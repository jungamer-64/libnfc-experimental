/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartière
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

/**
 * @file arygon.c
 * @brief ARYGON readers driver (Refactored for robustness)
 *
 * This driver handles ARYGON readers using UART communication.
 * UART connection can be direct (host<->arygon_uc) or via USB-to-serial
 * interface (e.g. host<->ftdi_chip<->arygon_uc)
 *
 * Refactoring improvements:
 * - Unified error handling with goto error pattern
 * - Enhanced input validation
 * - Reduced code duplication using nfc-common.h helpers
 * - Improved resource cleanup
 * - Better type safety and boundary checking
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif // HAVE_CONFIG_H

#include "arygon.h"

#include <stdio.h>
#include <inttypes.h>
#include <string.h>
#include <unistd.h>

#include <nfc/nfc.h>

#include "drivers.h"
#include "nfc-internal.h"
#include "nfc-secure.h"
#include "nfc-common.h"
#include "chips/pn53x.h"
#include "chips/pn53x-internal.h"
#include "uart.h"

/* ============================================================================
 * PROTOCOL DEFINITIONS
 * ========================================================================== */

/** @def DEV_ARYGON_PROTOCOL_ARYGON_ASCII
 * @brief High level language in ASCII format. (Common µC commands and Mifare® commands)
 */
#define DEV_ARYGON_PROTOCOL_ARYGON_ASCII '0'

/** @def DEV_ARYGON_PROTOCOL_ARYGON_BINARY_WAB
 * @brief High level language in Binary format With AddressingByte for party line.
 */
#define DEV_ARYGON_PROTOCOL_ARYGON_BINARY_WAB '1'

/** @def DEV_ARYGON_PROTOCOL_TAMA
 * @brief Philips protocol (TAMA language) in binary format.
 */
#define DEV_ARYGON_PROTOCOL_TAMA '2'

/** @def DEV_ARYGON_PROTOCOL_TAMA_WAB
 * @brief Philips protocol (TAMA language) in binary With AddressingByte for party line.
 */
#define DEV_ARYGON_PROTOCOL_TAMA_WAB '3'

/* ============================================================================
 * CONSTANTS
 * ========================================================================== */

#define ARYGON_DEFAULT_SPEED 9600
#define ARYGON_DRIVER_NAME "arygon"
#define ARYGON_FIRMWARE_VERSION_MAX_LEN 10
#define ARYGON_ERROR_FRAME_LEN 10

#define LOG_CATEGORY "libnfc.driver.arygon"
#define LOG_GROUP NFC_LOG_GROUP_DRIVER

#define DRIVER_DATA(pnd) ((struct arygon_data *)(pnd->driver_data))

/* ============================================================================
 * BUFFER SIZE DEFINITIONS
 * ========================================================================== */

#define ARYGON_TX_BUFFER_LEN (PN53x_NORMAL_FRAME__DATA_MAX_LEN + \
                              PN53x_NORMAL_FRAME__OVERHEAD + 1)
#define ARYGON_RX_BUFFER_LEN (PN53x_EXTENDED_FRAME__DATA_MAX_LEN + \
                              PN53x_EXTENDED_FRAME__OVERHEAD)

/* ============================================================================
 * DATA STRUCTURES
 * ========================================================================== */

/**
 * @struct arygon_data
 * @brief Internal data structure for ARYGON driver
 */
struct arygon_data
{
  serial_port port;
#ifndef WIN32
  int iAbortFds[2];
#else
  volatile bool abort_flag;
#endif
};

/**
 * @struct arygon_descriptor
 * @brief Connection descriptor for ARYGON device
 */
struct arygon_descriptor
{
  char *port;
  uint32_t speed;
};

/* ============================================================================
 * ERROR FRAMES
 * ========================================================================== */

static const uint8_t arygon_error_none[] = "FF000000\x0d\x0a";
static const uint8_t arygon_error_unknown_mode[] = "FF060000\x0d\x0a";

/* ============================================================================
 * FORWARD DECLARATIONS
 * ========================================================================== */

static int arygon_reset_tama(nfc_device *pnd);
static void arygon_firmware(nfc_device *pnd, char *str, size_t str_len);
static int arygon_tama_send(nfc_device *pnd, const uint8_t *pbtData,
                            const size_t szData, int timeout);
static int arygon_tama_receive(nfc_device *pnd, uint8_t *pbtData,
                               const size_t szDataLen, int timeout);
static int arygon_abort(nfc_device *pnd);
static void arygon_close_step2(nfc_device *pnd);

/* ============================================================================
 * I/O INTERFACE
 * ========================================================================== */

const struct pn53x_io arygon_tama_io = {
    .send = arygon_tama_send,
    .receive = arygon_tama_receive,
};

/* ============================================================================
 * HELPER FUNCTIONS
 * ========================================================================== */

/**
 * @brief Initialize ARYGON device structure with comprehensive error handling
 *
 * Centralizes device initialization with proper cleanup on failure.
 *
 * @param context NFC context
 * @param connstring Connection string
 * @param sp Serial port (already opened)
 * @return Initialized device or NULL on failure
 */
static nfc_device *
arygon_init_device(const nfc_context *context, const nfc_connstring connstring,
                   serial_port sp)
{
  /* Validate input parameters */
  if (!context || !connstring)
  {
    NFC_LOG_ERROR_AND_RETURN(NULL, "Invalid context or connstring");
  }

  if (sp == INVALID_SERIAL_PORT || sp == CLAIMED_SERIAL_PORT)
  {
    NFC_LOG_ERROR_AND_RETURN(NULL, "Invalid serial port handle");
  }

  /* Allocate device structure */
  nfc_device *pnd = nfc_device_new(context, connstring);
  if (!pnd)
  {
    NFC_LOG_ERROR_AND_RETURN(NULL, "Failed to allocate device structure");
  }

  pnd->driver = &arygon_driver;

  /* Allocate driver data using unified allocation */
  if (nfc_alloc_driver_data(pnd, sizeof(struct arygon_data)) < 0)
  {
    nfc_device_free(pnd);
    return NULL;
  }
  DRIVER_DATA(pnd)->port = sp;

  /* Allocate and initialize chip's data */
  if (pn53x_data_new(pnd, &arygon_tama_io) == NULL)
  {
    nfc_device_free(pnd);
    NFC_LOG_ERROR_AND_RETURN(NULL, "Failed to allocate chip data");
  }

  /* Initialize abort mechanism */
#ifndef WIN32
  if (nfc_init_abort_mechanism(DRIVER_DATA(pnd)->iAbortFds) < 0)
  {
    pn53x_data_free(pnd);
    nfc_device_free(pnd);
    return NULL;
  }
#else
  DRIVER_DATA(pnd)->abort_flag = false;
#endif

  return pnd;
}

/**
 * @brief Cleanup device on initialization failure
 *
 * @param pnd Device to cleanup
 * @param close_port Whether to close the serial port
 */
static void
arygon_cleanup_device(nfc_device *pnd, bool close_port)
{
  if (!pnd)
  {
    return;
  }

  if (close_port && DRIVER_DATA(pnd)->port != INVALID_SERIAL_PORT)
  {
    uart_close(DRIVER_DATA(pnd)->port);
  }

#ifndef WIN32
  nfc_close_abort_mechanism(DRIVER_DATA(pnd)->iAbortFds);
#endif

  pn53x_data_free(pnd);
  nfc_device_free(pnd);
}

/**
 * @brief Validate and open serial port with enhanced error checking
 *
 * @param port_name Port name to open
 * @param speed Baud rate
 * @return Opened serial port or INVALID_SERIAL_PORT on failure
 */
static serial_port
arygon_open_port(const char *port_name, uint32_t speed)
{
  /* Input validation */
  if (!port_name)
  {
    NFC_LOG_ERROR_AND_RETURN(INVALID_SERIAL_PORT, "NULL port name");
  }

  /* Validate speed is reasonable (typical UART speeds) */
  if (speed == 0 || speed > 115200)
  {
    NFC_LOG_ERROR_AND_RETURN(INVALID_SERIAL_PORT,
                             "Invalid baud rate: %d", speed);
  }

  NFC_LOG_DEBUG("Attempting to open: %s at %d baud", port_name, speed);

  serial_port sp = uart_open(port_name);

  if (sp == INVALID_SERIAL_PORT)
  {
    NFC_LOG_ERROR_AND_RETURN(INVALID_SERIAL_PORT,
                             "Invalid serial port: %s", port_name);
  }

  if (sp == CLAIMED_SERIAL_PORT)
  {
    NFC_LOG_ERROR_AND_RETURN(CLAIMED_SERIAL_PORT,
                             "Serial port already claimed: %s", port_name);
  }

  /* Flush input to ensure clean communication */
  uart_flush_input(sp, true);
  uart_set_speed(sp, speed);

  return sp;
}

/* ============================================================================
 * DEVICE SCANNING
 * ========================================================================== */

static size_t
arygon_scan(const nfc_context *context, nfc_connstring connstrings[],
            const size_t connstrings_len)
{
  /* Input validation */
  if (!context || !connstrings || connstrings_len == 0)
  {
    return 0;
  }

  size_t device_found = 0;
  char **acPorts = uart_list_ports();
  if (!acPorts)
  {
    return 0;
  }

  for (int iDevice = 0; acPorts[iDevice] != NULL; iDevice++)
  {
    const char *acPort = acPorts[iDevice];

    /* Try to open port */
    serial_port sp = arygon_open_port(acPort, ARYGON_DEFAULT_SPEED);
    if (sp == INVALID_SERIAL_PORT || sp == CLAIMED_SERIAL_PORT)
    {
      continue;
    }

    /* Build connection string */
    nfc_connstring connstring;
    int ret = snprintf(connstring, sizeof(nfc_connstring), "%s:%s:%" PRIu32,
                       ARYGON_DRIVER_NAME, acPort, ARYGON_DEFAULT_SPEED);

    /* Check for truncation */
    if (ret < 0 || (size_t)ret >= sizeof(nfc_connstring))
    {
      NFC_LOG_WARN("Connection string truncated for port: %s", acPort);
      uart_close(sp);
      continue;
    }

    /* Initialize device structure */
    nfc_device *pnd = arygon_init_device(context, connstring, sp);
    if (!pnd)
    {
      uart_close(sp);
      continue;
    }

    /* Test communication with device */
    int res = arygon_reset_tama(pnd);
    arygon_cleanup_device(pnd, true);

    if (res < 0)
    {
      continue;
    }

    /* ARYGON reader found - copy connection string */
    if (nfc_copy_connstring(connstrings[device_found], connstring) < 0)
    {
      continue;
    }

    device_found++;

    /* Check if we've found enough devices */
    if (device_found >= connstrings_len)
    {
      break;
    }
  }

  nfc_free_array((void **)acPorts);
  return device_found;
}

/* ============================================================================
 * DEVICE OPEN/CLOSE
 * ========================================================================== */

static nfc_device *
arygon_open(const nfc_context *context, const nfc_connstring connstring)
{
  /* Input validation */
  if (!context || !connstring)
  {
    return NULL;
  }

  /* Parse connection string */
  struct arygon_descriptor ndd = {0};
  char *speed_s = NULL;
  serial_port sp = INVALID_SERIAL_PORT;
  nfc_device *pnd = NULL;

  int decode_level = connstring_decode(connstring, ARYGON_DRIVER_NAME,
                                       NULL, &ndd.port, &speed_s);

  if (decode_level < 2)
  {
    return NULL;
  }

  /* Parse speed if provided */
  if (decode_level == 3)
  {
    if (sscanf(speed_s, "%10" PRIu32, &ndd.speed) != 1)
    {
      NFC_LOG_ERROR("Invalid speed format: %s", speed_s);
      goto error;
    }
  }
  else
  {
    ndd.speed = ARYGON_DEFAULT_SPEED;
  }

  /* Open serial port */
  sp = arygon_open_port(ndd.port, ndd.speed);
  if (sp == INVALID_SERIAL_PORT || sp == CLAIMED_SERIAL_PORT)
  {
    goto error;
  }

  /* Create device structure */
  pnd = arygon_init_device(context, connstring, sp);
  if (!pnd)
  {
    uart_close(sp);
    goto error;
  }

  /* Set device name with bounds checking */
  int name_len = snprintf(pnd->name, sizeof(pnd->name), "%s:%s",
                          ARYGON_DRIVER_NAME, ndd.port);
  if (name_len < 0 || (size_t)name_len >= sizeof(pnd->name))
  {
    NFC_LOG_WARN("Device name truncated");
  }

  /* Configure chip-specific settings */
  CHIP_DATA(pnd)->power_mode = NORMAL;
  CHIP_DATA(pnd)->timer_correction = 46; // Empirical tuning

  /* Verify communication */
  if (arygon_reset_tama(pnd) < 0)
  {
    arygon_cleanup_device(pnd, true);
    pnd = NULL;
    goto error;
  }

  /* Get firmware version and update device name */
  char arygon_firmware_version[ARYGON_FIRMWARE_VERSION_MAX_LEN];
  arygon_firmware(pnd, arygon_firmware_version, sizeof(arygon_firmware_version));

  char *pcName = strdup(pnd->name);
  if (pcName)
  {
    snprintf(pnd->name, sizeof(pnd->name), "%s %s",
             pcName, arygon_firmware_version);
    free(pcName);
  }

  pn53x_init(pnd);

error:
  /* Cleanup temporary allocations */
  free(ndd.port);
  free(speed_s);

  return pnd;
}

static void
arygon_close_step2(nfc_device *pnd)
{
  if (!pnd)
  {
    return;
  }

  /* Release UART port */
  if (DRIVER_DATA(pnd)->port != INVALID_SERIAL_PORT)
  {
    uart_close(DRIVER_DATA(pnd)->port);
  }

#ifndef WIN32
  nfc_close_abort_mechanism(DRIVER_DATA(pnd)->iAbortFds);
#endif

  pn53x_data_free(pnd);
  nfc_device_free(pnd);
}

static void
arygon_close(nfc_device *pnd)
{
  if (!nfc_device_validate(pnd, "arygon_close"))
  {
    return;
  }

  pn53x_idle(pnd);
  arygon_close_step2(pnd);
}

/* ============================================================================
 * TAMA PROTOCOL IMPLEMENTATION
 * ========================================================================== */

static int
arygon_tama_send(nfc_device *pnd, const uint8_t *pbtData,
                 const size_t szData, int timeout)
{
  /* Comprehensive input validation */
  if (!nfc_device_validate(pnd, "arygon_tama_send"))
  {
    return NFC_EINVARG;
  }

  if (!pbtData)
  {
    pnd->last_error = NFC_EINVARG;
    NFC_LOG_ERROR_AND_RETURN(NFC_EINVARG, "NULL data pointer");
  }

  if (szData == 0)
  {
    pnd->last_error = NFC_EINVARG;
    NFC_LOG_ERROR_AND_RETURN(NFC_EINVARG, "Zero-size data");
  }

  /* Validate data size */
  if (szData > PN53x_NORMAL_FRAME__DATA_MAX_LEN)
  {
    pnd->last_error = NFC_EDEVNOTSUPP;
    NFC_LOG_ERROR_AND_RETURN(NFC_EDEVNOTSUPP,
                             "ARYGON device does not support more than %d bytes "
                             "as payload (requested: %zu)",
                             PN53x_NORMAL_FRAME__DATA_MAX_LEN, szData);
  }

  /* Flush input before sending */
  uart_flush_input(DRIVER_DATA(pnd)->port, false);

  /* Build frame */
  uint8_t abtFrame[ARYGON_TX_BUFFER_LEN] = {
      DEV_ARYGON_PROTOCOL_TAMA, 0x00, 0x00, 0xff};

  size_t szFrame = 0;
  int res = pn53x_build_frame(abtFrame + 1, &szFrame, pbtData, szData);
  if (res < 0)
  {
    pnd->last_error = res;
    return res;
  }

  /* Boundary check before sending */
  if (szFrame + 1 > sizeof(abtFrame))
  {
    pnd->last_error = NFC_ESOFT;
    NFC_LOG_ERROR_AND_RETURN(NFC_ESOFT, "Frame size exceeds buffer");
  }

  /* Send frame */
  res = uart_send(DRIVER_DATA(pnd)->port, abtFrame, szFrame + 1, timeout);
  if (res != 0)
  {
    pnd->last_error = res;
    NFC_LOG_ERROR_AND_RETURN(res, "Unable to transmit data (TX)");
  }

  /* Receive ACK */
  uint8_t abtRxBuf[PN53x_ACK_FRAME__LEN];
  res = uart_receive(DRIVER_DATA(pnd)->port, abtRxBuf,
                     sizeof(abtRxBuf), 0, timeout);
  if (res != 0)
  {
    pnd->last_error = res;
    NFC_LOG_ERROR_AND_RETURN(res, "Unable to read ACK");
  }

  /* Verify ACK frame */
  if (pn53x_check_ack_frame(pnd, abtRxBuf, sizeof(abtRxBuf)) == 0)
  {
    return NFC_SUCCESS;
  }

  /* Check for error frame */
  if (memcmp(arygon_error_unknown_mode, abtRxBuf, sizeof(abtRxBuf)) == 0)
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR("Bad frame format");
    /* Read remaining 4 bytes to synchronize */
    uart_receive(DRIVER_DATA(pnd)->port, abtRxBuf, 4, 0, timeout);
    return NFC_EIO;
  }

  return pnd->last_error;
}

static int
arygon_tama_receive(nfc_device *pnd, uint8_t *pbtData,
                    const size_t szDataLen, int timeout)
{
  /* Input validation */
  if (!nfc_device_validate(pnd, "arygon_tama_receive"))
  {
    return NFC_EINVARG;
  }

  if (!pbtData)
  {
    pnd->last_error = NFC_EINVARG;
    NFC_LOG_ERROR_AND_RETURN(NFC_EINVARG, "NULL data buffer");
  }

  if (szDataLen == 0)
  {
    pnd->last_error = NFC_EINVARG;
    NFC_LOG_ERROR_AND_RETURN(NFC_EINVARG, "Zero-size buffer");
  }

  uint8_t abtRxBuf[5];
  void *abort_p = NULL;

#ifndef WIN32
  abort_p = &(DRIVER_DATA(pnd)->iAbortFds[1]);
#else
  abort_p = (void *)&(DRIVER_DATA(pnd)->abort_flag);
#endif

  /* Receive preamble and length */
  pnd->last_error = uart_receive(DRIVER_DATA(pnd)->port, abtRxBuf, 5,
                                 abort_p, timeout);

  if (abort_p && (NFC_EOPABORTED == pnd->last_error))
  {
    arygon_abort(pnd);
    return NFC_EOPABORTED;
  }

  if (pnd->last_error != 0)
  {
    NFC_LOG_ERROR_AND_RETURN(pnd->last_error, "Unable to receive data (RX)");
  }

  /* Verify preamble */
  const uint8_t pn53x_preamble[3] = {0x00, 0x00, 0xff};
  if (memcmp(abtRxBuf, pn53x_preamble, 3) != 0)
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO, "Frame preamble+start code mismatch");
  }

  /* Check for error frame */
  if ((0x01 == abtRxBuf[3]) && (0xff == abtRxBuf[4]))
  {
    uart_receive(DRIVER_DATA(pnd)->port, abtRxBuf, 3, 0, timeout);
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO, "Application level error detected");
  }

  /* Check for extended frame (not supported) */
  if ((0xff == abtRxBuf[3]) && (0xff == abtRxBuf[4]))
  {
    pnd->last_error = NFC_EDEVNOTSUPP;
    NFC_LOG_ERROR_AND_RETURN(NFC_EDEVNOTSUPP, "Extended frames not supported");
  }

  /* Validate length checksum */
  if (256 != (abtRxBuf[3] + abtRxBuf[4]))
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO, "Length checksum mismatch");
  }

  /* Calculate data length (LEN includes TFI + CC+1) */
  if (abtRxBuf[3] < 2)
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO, "Invalid frame length: %d", abtRxBuf[3]);
  }

  size_t len = abtRxBuf[3] - 2;

  if (len > szDataLen)
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO,
                             "Buffer too small (szDataLen: %zu, len: %zu)",
                             szDataLen, len);
  }

  /* Receive TFI and command code */
  pnd->last_error = uart_receive(DRIVER_DATA(pnd)->port, abtRxBuf, 2, 0, timeout);
  if (pnd->last_error != 0)
  {
    NFC_LOG_ERROR_AND_RETURN(pnd->last_error, "Unable to receive TFI (RX)");
  }

  /* Verify TFI */
  if (abtRxBuf[0] != 0xD5)
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO, "TFI mismatch (expected 0xD5, got 0x%02X)",
                             abtRxBuf[0]);
  }

  /* Verify command code */
  if (abtRxBuf[1] != CHIP_DATA(pnd)->last_command + 1)
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO,
                             "Command code verification failed "
                             "(expected 0x%02X, got 0x%02X)",
                             CHIP_DATA(pnd)->last_command + 1, abtRxBuf[1]);
  }

  /* Receive payload data */
  if (len > 0)
  {
    pnd->last_error = uart_receive(DRIVER_DATA(pnd)->port, pbtData, len, 0, timeout);
    if (pnd->last_error != 0)
    {
      NFC_LOG_ERROR_AND_RETURN(pnd->last_error, "Unable to receive payload (RX)");
    }
  }

  /* Receive DCS and postamble */
  pnd->last_error = uart_receive(DRIVER_DATA(pnd)->port, abtRxBuf, 2, 0, timeout);
  if (pnd->last_error != 0)
  {
    NFC_LOG_ERROR_AND_RETURN(pnd->last_error, "Unable to receive DCS (RX)");
  }

  /* Verify data checksum */
  uint8_t btDCS = (256 - 0xD5);
  btDCS -= CHIP_DATA(pnd)->last_command + 1;
  for (size_t szPos = 0; szPos < len; szPos++)
  {
    btDCS -= pbtData[szPos];
  }

  if (btDCS != abtRxBuf[0])
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO,
                             "Data checksum mismatch (expected 0x%02X, got 0x%02X)",
                             btDCS, abtRxBuf[0]);
  }

  /* Verify postamble */
  if (0x00 != abtRxBuf[1])
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_ERROR_AND_RETURN(NFC_EIO,
                             "Frame postamble mismatch (expected 0x00, got 0x%02X)",
                             abtRxBuf[1]);
  }

  return (int)len;
}

/* ============================================================================
 * ARYGON-SPECIFIC COMMANDS
 * ========================================================================== */

static void
arygon_firmware(nfc_device *pnd, char *str, size_t str_len)
{
  /* Input validation */
  if (!pnd || !str || str_len == 0)
  {
    return;
  }

  const uint8_t arygon_firmware_version_cmd[] = {
      DEV_ARYGON_PROTOCOL_ARYGON_ASCII, 'a', 'v'};
  uint8_t abtRx[16];
  size_t szRx = sizeof(abtRx);

  int res = uart_send(DRIVER_DATA(pnd)->port, arygon_firmware_version_cmd,
                      sizeof(arygon_firmware_version_cmd), 0);
  if (res != 0)
  {
    NFC_LOG_DEBUG("Unable to send ARYGON firmware command");
    str[0] = '\0';
    return;
  }

  res = uart_receive(DRIVER_DATA(pnd)->port, abtRx, szRx, 0, 0);
  if (res != 0)
  {
    NFC_LOG_DEBUG("Unable to retrieve ARYGON firmware version");
    str[0] = '\0';
    return;
  }

  if (memcmp(abtRx, arygon_error_none, 6) == 0)
  {
    uint8_t *p = abtRx + 6;
    unsigned int szData;

    if (sscanf((const char *)p, "%02x", &szData) != 1)
    {
      str[0] = '\0';
      return;
    }

    /* Advance pointer past the size field (2 hex digits) */
    p += 2;

    /* Boundary check: ensure szData doesn't exceed buffer bounds */
    const size_t max_copy_len = MIN(szData, str_len - 1);
    const size_t remaining_rx = sizeof(abtRx) - 8; // 6 (header) + 2 (size field)

    if (szData > remaining_rx)
    {
      NFC_LOG_WARN("Firmware version data truncated (%u > %zu)",
                   szData, remaining_rx);
      str[0] = '\0';
      return;
    }

    if (nfc_safe_memcpy(str, str_len, p, max_copy_len) < 0)
    {
      NFC_LOG_ERROR("Failed to copy firmware version");
      str[0] = '\0';
      return;
    }
    str[max_copy_len] = '\0';
  }
  else
  {
    str[0] = '\0';
  }
}

static int
arygon_reset_tama(nfc_device *pnd)
{
  if (!nfc_device_validate(pnd, "arygon_reset_tama"))
  {
    return NFC_EINVARG;
  }

  const uint8_t arygon_reset_tama_cmd[] = {
      DEV_ARYGON_PROTOCOL_ARYGON_ASCII, 'a', 'r'};
  uint8_t abtRx[ARYGON_ERROR_FRAME_LEN];

  uart_send(DRIVER_DATA(pnd)->port, arygon_reset_tama_cmd,
            sizeof(arygon_reset_tama_cmd), 500);

  int res = uart_receive(DRIVER_DATA(pnd)->port, abtRx, sizeof(abtRx), 0, 1000);
  if (res != 0)
  {
    NFC_LOG_DEBUG("No reply to 'reset TAMA' command");
    pnd->last_error = res;
    return pnd->last_error;
  }

  if (memcmp(abtRx, arygon_error_none, sizeof(arygon_error_none) - 1) != 0)
  {
    pnd->last_error = NFC_EIO;
    NFC_LOG_DEBUG("Reset TAMA failed - invalid response");
    return pnd->last_error;
  }

  return NFC_SUCCESS;
}

static int
arygon_abort(nfc_device *pnd)
{
  if (!nfc_device_validate(pnd, "arygon_abort"))
  {
    return NFC_EINVARG;
  }

  /* Send a valid TAMA packet to wake up the PN53x
   * (we will not have an answer, according to Arygon manual) */
  const uint8_t dummy[] = {
      0x32, 0x00, 0x00, 0xff, 0x09, 0xf7, 0xd4, 0x00,
      0x00, 0x6c, 0x69, 0x62, 0x6e, 0x66, 0x63, 0xbe, 0x00};

  uart_send(DRIVER_DATA(pnd)->port, dummy, sizeof(dummy), 0);

  /* Verify communication is restored */
  return pn53x_check_communication(pnd);
}

static int
arygon_abort_command(nfc_device *pnd)
{
  if (!pnd)
  {
    return NFC_EINVARG;
  }

#ifndef WIN32
  /* Reset abort pipe - close old pipe first */
  close(DRIVER_DATA(pnd)->iAbortFds[0]);

  /* Create new pipe for abort mechanism */
  if (pipe(DRIVER_DATA(pnd)->iAbortFds) < 0)
  {
    NFC_LOG_ERROR_AND_RETURN(NFC_ESOFT, "Failed to recreate abort pipe");
  }
#else
  DRIVER_DATA(pnd)->abort_flag = true;
#endif

  return NFC_SUCCESS;
}

/* ============================================================================
 * DRIVER INTERFACE
 * ========================================================================== */

const struct nfc_driver arygon_driver = {
    .name = ARYGON_DRIVER_NAME,
    .scan_type = INTRUSIVE,
    .scan = arygon_scan,
    .open = arygon_open,
    .close = arygon_close,
    .strerror = pn53x_strerror,

    .initiator_init = pn53x_initiator_init,
    .initiator_init_secure_element = NULL, // No secure-element support
    .initiator_select_passive_target = pn53x_initiator_select_passive_target,
    .initiator_poll_target = pn53x_initiator_poll_target,
    .initiator_select_dep_target = pn53x_initiator_select_dep_target,
    .initiator_deselect_target = pn53x_initiator_deselect_target,
    .initiator_transceive_bytes = pn53x_initiator_transceive_bytes,
    .initiator_transceive_bits = pn53x_initiator_transceive_bits,
    .initiator_transceive_bytes_timed = pn53x_initiator_transceive_bytes_timed,
    .initiator_transceive_bits_timed = pn53x_initiator_transceive_bits_timed,
    .initiator_target_is_present = pn53x_initiator_target_is_present,

    .target_init = pn53x_target_init,
    .target_send_bytes = pn53x_target_send_bytes,
    .target_receive_bytes = pn53x_target_receive_bytes,
    .target_send_bits = pn53x_target_send_bits,
    .target_receive_bits = pn53x_target_receive_bits,

    .device_set_property_bool = pn53x_set_property_bool,
    .device_set_property_int = pn53x_set_property_int,
    .get_supported_modulation = pn53x_get_supported_modulation,
    .get_supported_baud_rate = pn53x_get_supported_baud_rate,
    .device_get_information_about = pn53x_get_information_about,

    .abort_command = arygon_abort_command,
    .idle = pn53x_idle,
    /* Even if PN532, PowerDown is not recommended on those devices */
    .powerdown = NULL,
}
