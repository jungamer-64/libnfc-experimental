/*-
 * Free/Libre Near Field Communication (NFC) library
 * Refactored for improved type safety, modularity, and maintainability
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif

#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
#include <assert.h>
#include <ctype.h>

#ifndef HAVE_STRNLEN
extern size_t strnlen(const char *s, size_t maxlen);
#endif

#include <nfc/nfc.h>
#include "nfc-internal.h"
#include "nfc-secure.h"
#include "target-subr.h"
#include "drivers.h"

#ifndef PACKAGE_VERSION
#define PACKAGE_VERSION "unknown"
#endif

/* ============================================================================
 * CONSTANTS AND TYPE DEFINITIONS
 * ========================================================================== */

#define LOG_CATEGORY "libnfc.general"
#define LOG_GROUP NFC_LOG_GROUP_GENERAL
#define NFC_DRIVER_NAME_MAX 64

/* Device open results - improved type safety */
typedef enum
{
  NFC_DRIVER_SKIP,
  NFC_DRIVER_OPENED,
  NFC_DRIVER_ABORT
} nfc_driver_open_result_t;

/* Property setting structure for bulk configuration */
typedef struct
{
  nfc_property property;
  bool value;
} property_bool_setting_t;

/* Error message mapping structure */
typedef struct
{
  int error_code;
  const char *error_msg;
} error_message_t;

/* ============================================================================
 * DRIVER LIST MANAGEMENT TYPES
 * ========================================================================== */

struct nfc_driver_list
{
  const struct nfc_driver_list *next;
  const struct nfc_driver *driver;
};

/* ============================================================================
 * STATIC DATA
 * ========================================================================== */

const struct nfc_driver_list *nfc_drivers = NULL;

static const char *nfc_property_name[] = {
    "NP_TIMEOUT_COMMAND",
    "NP_TIMEOUT_ATR",
    "NP_TIMEOUT_COM",
    "NP_HANDLE_CRC",
    "NP_HANDLE_PARITY",
    "NP_ACTIVATE_FIELD",
    "NP_ACTIVATE_CRYPTO1",
    "NP_INFINITE_SELECT",
    "NP_ACCEPT_INVALID_FRAMES",
    "NP_ACCEPT_MULTIPLE_FRAMES",
    "NP_AUTO_ISO14443_4",
    "NP_EASY_FRAMING",
    "NP_FORCE_ISO14443_A",
    "NP_FORCE_ISO14443_B",
    "NP_FORCE_SPEED_106"};

static const error_message_t ERROR_MESSAGES[] = {
    {NFC_SUCCESS, "Success"},
    {NFC_EIO, "Input / Output Error"},
    {NFC_EINVARG, "Invalid argument(s)"},
    {NFC_EDEVNOTSUPP, "Not Supported by Device"},
    {NFC_ENOTSUCHDEV, "No Such Device"},
    {NFC_EOVFLOW, "Buffer Overflow"},
    {NFC_ETIMEOUT, "Timeout"},
    {NFC_EOPABORTED, "Operation Aborted"},
    {NFC_ENOTIMPL, "Not (yet) Implemented"},
    {NFC_ETGRELEASED, "Target Released"},
    {NFC_EMFCAUTHFAIL, "Mifare Authentication Failed"},
    {NFC_ERFTRANS, "RF Transmission Error"},
    {NFC_ECHIP, "Device's Internal Chip Error"},
};

/* Modulation type name mapping */
typedef struct
{
  nfc_modulation_type type;
  const char *name;
} modulation_type_name_t;

static const modulation_type_name_t MODULATION_TYPE_NAMES[] = {
    {NMT_ISO14443A, "ISO/IEC 14443A"},
    {NMT_ISO14443B, "ISO/IEC 14443-4B"},
    {NMT_ISO14443BI, "ISO/IEC 14443-4B'"},
    {NMT_ISO14443BICLASS, "ISO/IEC 14443-2B-3B iClass (Picopass)"},
    {NMT_ISO14443B2CT, "ISO/IEC 14443-2B ASK CTx"},
    {NMT_ISO14443B2SR, "ISO/IEC 14443-2B ST SRx"},
    {NMT_FELICA, "FeliCa"},
    {NMT_JEWEL, "Innovision Jewel"},
    {NMT_BARCODE, "Thinfilm NFC Barcode"},
    {NMT_DEP, "D.E.P."}};

/* ============================================================================
 * FORWARD DECLARATIONS
 * ========================================================================== */

static int nfc_device_validate_modulation(
    nfc_device *pnd,
    const nfc_mode mode,
    const nfc_modulation *nm);

/* ============================================================================
 * STRING VALIDATION UTILITIES
 * ========================================================================== */

static inline bool
string_contains_control_chars(const char *value, size_t length)
{
  if (!value)
    return false;

  for (size_t i = 0; i < length && value[i]; i++)
  {
    if (!isprint((unsigned char)value[i]))
    {
      return true;
    }
  }
  return false;
}

static inline bool
string_is_numeric(const char *value, size_t length)
{
  if (!value || length == 0)
    return false;

  for (size_t i = 0; i < length && value[i]; i++)
  {
    if (!isdigit((unsigned char)value[i]))
    {
      return false;
    }
  }
  return true;
}

/* ============================================================================
 * CONNECTION STRING UTILITIES
 * ========================================================================== */

static bool
copy_connstring_safely(const char *source, nfc_connstring destination)
{
  if (!source)
    return false;

  const size_t length = nfc_safe_strlen(source, NFC_BUFSIZE_CONNSTRING);

  if (string_contains_control_chars(source, length))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Connection string contains control characters");
    return false;
  }

  if (length >= NFC_BUFSIZE_CONNSTRING)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Connection string exceeds maximum length");
    return false;
  }

  if (nfc_safe_memcpy(destination, NFC_BUFSIZE_CONNSTRING,
                      source, length + 1) < 0)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Failed to copy connection string");
    return false;
  }

  destination[length] = '\0';
  return true;
}

static inline bool
connstring_is_usb_request(const nfc_connstring ncs)
{
  return strncmp(ncs, "usb", 3) == 0;
}

static bool
prepare_connstring(
    nfc_context *context,
    const nfc_connstring connstring,
    nfc_connstring destination)
{
  if (!connstring)
  {
    nfc_connstring discovered;
    if (nfc_list_devices(context, &discovered, 1) == 0)
    {
      return false;
    }
    return copy_connstring_safely(discovered, destination);
  }

  return copy_connstring_safely(connstring, destination);
}

/* ============================================================================
 * DRIVER MANAGEMENT
 * ========================================================================== */

static void
nfc_drivers_init(void)
{
#if defined(DRIVER_PN53X_USB_ENABLED)
  nfc_register_driver(&pn53x_usb_driver);
#endif
#if defined(DRIVER_PCSC_ENABLED)
  nfc_register_driver(&pcsc_driver);
#endif
#if defined(DRIVER_PN532_UART_ENABLED)
  nfc_register_driver(&pn532_uart_driver);
#endif
#if defined(DRIVER_PN532_SPI_ENABLED)
  nfc_register_driver(&pn532_spi_driver);
#endif
#if defined(DRIVER_PN532_I2C_ENABLED)
  nfc_register_driver(&pn532_i2c_driver);
#endif
#if defined(DRIVER_ARYGON_ENABLED)
  nfc_register_driver(&arygon_driver);
#endif
}

int nfc_register_driver(const struct nfc_driver *ndr)
{
  if (!ndr)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "nfc_register_driver: NULL driver");
    return NFC_EINVARG;
  }

  struct nfc_driver_list *pndl =
      (struct nfc_driver_list *)malloc(sizeof(struct nfc_driver_list));

  if (!pndl)
    return NFC_ESOFT;

  pndl->driver = ndr;
  pndl->next = nfc_drivers;
  nfc_drivers = pndl;

  return NFC_SUCCESS;
}

static inline bool
driver_matches_connstring(
    const struct nfc_driver *ndr,
    const nfc_connstring ncs,
    bool request_is_usb)
{
  if (!ndr || !ndr->name)
    return false;

  const size_t name_len = nfc_safe_strlen(ndr->name, NFC_DRIVER_NAME_MAX);

  if (strncmp(ndr->name, ncs, name_len) == 0)
  {
    return true;
  }

  /* USB driver matching: check for "_usb" suffix */
  if (request_is_usb && name_len >= 4)
  {
    return strncmp(ndr->name + (name_len - 4), "_usb", 4) == 0;
  }

  return false;
}

/* ============================================================================
 * DEVICE OPENING LOGIC
 * ========================================================================== */

static bool
apply_user_defined_device_name(
    nfc_context *context,
    const nfc_connstring ncs,
    nfc_device *pnd)
{
  if (!context || !pnd)
    return true;

  for (uint32_t i = 0; i < context->user_defined_device_count; i++)
  {
    if (strcmp(ncs, context->user_defined_devices[i].connstring) != 0)
    {
      continue;
    }

    const size_t name_len = nfc_safe_strlen(
        context->user_defined_devices[i].name,
        DEVICE_NAME_LENGTH);

    if (nfc_safe_memcpy(pnd->name, DEVICE_NAME_LENGTH,
                        context->user_defined_devices[i].name,
                        name_len) < 0)
    {
      return false;
    }

    pnd->name[name_len] = '\0';
    break;
  }
  return true;
}

static nfc_driver_open_result_t
attempt_open_driver(
    nfc_context *context,
    const nfc_connstring ncs,
    bool request_is_usb,
    const struct nfc_driver *ndr,
    nfc_device **out_device)
{
  if (!driver_matches_connstring(ndr, ncs, request_is_usb))
  {
    return NFC_DRIVER_SKIP;
  }

  nfc_device *candidate = ndr->open(context, ncs);

  if (!candidate)
  {
    if (request_is_usb)
    {
      return NFC_DRIVER_SKIP;
    }
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "Unable to open \"%s\".", ncs);
    return NFC_DRIVER_ABORT;
  }

  *out_device = candidate;
  return NFC_DRIVER_OPENED;
}

static bool
finalize_opened_device(
    nfc_context *context,
    const nfc_connstring ncs,
    nfc_device *pnd)
{
  if (!apply_user_defined_device_name(context, ncs, pnd))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Failed to copy device name");
    nfc_close(pnd);
    return false;
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "\"%s\" (%s) has been claimed.", pnd->name, pnd->connstring);
  return true;
}

nfc_device *
nfc_open(nfc_context *context, const nfc_connstring connstring)
{
  nfc_connstring ncs;
  if (!prepare_connstring(context, connstring, ncs))
  {
    return NULL;
  }

  const bool request_is_usb = connstring_is_usb_request(ncs);

  for (const struct nfc_driver_list *pndl = nfc_drivers;
       pndl;
       pndl = pndl->next)
  {
    nfc_device *candidate = NULL;
    const nfc_driver_open_result_t result = attempt_open_driver(
        context, ncs, request_is_usb, pndl->driver, &candidate);

    switch (result)
    {
    case NFC_DRIVER_SKIP:
      continue;

    case NFC_DRIVER_ABORT:
      return NULL;

    case NFC_DRIVER_OPENED:
      if (candidate && finalize_opened_device(context, ncs, candidate))
      {
        return candidate;
      }
      return NULL;
    }
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "No driver available to handle \"%s\".", ncs);
  return NULL;
}

/* ============================================================================
 * DEVICE SCANNING AND ENUMERATION
 * ========================================================================== */

#ifdef CONFFILES
static bool
copy_connstring_entry(
    nfc_connstring connstrings[],
    size_t index,
    const nfc_connstring source)
{
  return copy_connstring_safely(source, connstrings[index]);
}

#ifdef ENVVARS
static char *
duplicate_log_level_env(void)
{
  char *env_log_level = getenv("LIBNFC_LOG_LEVEL");
  if (!env_log_level)
    return NULL;

  const size_t env_len = nfc_safe_strlen(env_log_level, 256);

  if (env_len >= 256)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN,
            "LIBNFC_LOG_LEVEL value is too long");
    return NULL;
  }

  if (!string_is_numeric(env_log_level, env_len) ||
      string_contains_control_chars(env_log_level, env_len))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN,
            "Ignoring invalid LIBNFC_LOG_LEVEL value");
    return NULL;
  }

  char *copy = malloc(env_len + 1);
  if (!copy)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Unable to malloc()");
    return NULL;
  }

  if (nfc_safe_memcpy(copy, env_len + 1, env_log_level, env_len) < 0)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Failed to copy log level");
    free(copy);
    return NULL;
  }

  copy[env_len] = '\0';
  return copy;
}

static void
restore_log_level_env(char *old_value, bool had_env)
{
  if (old_value)
  {
    if (setenv("LIBNFC_LOG_LEVEL", old_value, 1) != 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN,
              "Unable to restore LIBNFC_LOG_LEVEL");
    }
    free(old_value);
  }
  else if (!had_env)
  {
    if (unsetenv("LIBNFC_LOG_LEVEL") != 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN,
              "Unable to unset LIBNFC_LOG_LEVEL");
    }
  }
}
#endif // ENVVARS

static bool
optional_device_available(
    nfc_context *context,
    const struct nfc_user_defined_device *device)
{
#ifdef ENVVARS
  const char *current_log_level = getenv("LIBNFC_LOG_LEVEL");
  const bool had_env = current_log_level != NULL;
  char *old_env_log_level = duplicate_log_level_env();

  if (!had_env || old_env_log_level)
  {
    if (setenv("LIBNFC_LOG_LEVEL", "0", 1) != 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN,
              "Unable to reduce log verbosity when probing optional device");
    }
  }
#endif

  nfc_device *pnd = nfc_open(context, device->connstring);

#ifdef ENVVARS
  restore_log_level_env(old_env_log_level, had_env);
#endif

  if (!pnd)
    return false;

  nfc_close(pnd);
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "User device %s found", device->name);
  return true;
}

static size_t
append_user_defined_devices(
    nfc_context *context,
    nfc_connstring connstrings[],
    const size_t connstrings_len)
{
  size_t device_found = 0;

  for (uint32_t i = 0;
       i < context->user_defined_device_count && device_found < connstrings_len;
       i++)
  {
    const struct nfc_user_defined_device *device =
        &context->user_defined_devices[i];

    if (device->optional && !optional_device_available(context, device))
    {
      continue;
    }

    if (copy_connstring_entry(connstrings, device_found, device->connstring))
    {
      device_found++;
    }
  }

  return device_found;
}
#endif // CONFFILES

static inline bool
scan_allowed_for_driver(const nfc_context *context, const struct nfc_driver *ndr)
{
  if (!ndr)
    return false;

  return (ndr->scan_type == NOT_INTRUSIVE) ||
         (context->allow_intrusive_scan && (ndr->scan_type == INTRUSIVE));
}

static size_t
autoscan_devices(
    nfc_context *context,
    nfc_connstring connstrings[],
    size_t start_index,
    const size_t connstrings_len)
{
  size_t device_found = start_index;

  for (const struct nfc_driver_list *pndl = nfc_drivers;
       pndl && device_found < connstrings_len;
       pndl = pndl->next)
  {
    const struct nfc_driver *ndr = pndl->driver;

    if (!scan_allowed_for_driver(context, ndr))
    {
      continue;
    }

    const size_t remaining = connstrings_len - device_found;
    const size_t newly_found = ndr->scan(
        context,
        connstrings + device_found,
        remaining);

    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "%ld device(s) found using %s driver",
            (unsigned long)newly_found, ndr->name);

    if (newly_found > 0)
    {
      device_found += newly_found;
    }
  }

  return device_found;
}

size_t
nfc_list_devices(
    nfc_context *context,
    nfc_connstring connstrings[],
    const size_t connstrings_len)
{
  if (!context || !connstrings || connstrings_len == 0)
  {
    return 0;
  }

  size_t device_found = 0;

#ifdef CONFFILES
  device_found = append_user_defined_devices(context, connstrings, connstrings_len);
  if (device_found >= connstrings_len)
  {
    return device_found;
  }
#endif

  if (!context->allow_autoscan)
  {
    if (context->user_defined_device_count == 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO,
              "Warning: user must specify device(s) manually when autoscan is disabled");
    }
    return device_found;
  }

  return autoscan_devices(context, connstrings, device_found, connstrings_len);
}

/* ============================================================================
 * PROPERTY MANAGEMENT
 * ========================================================================== */

static int
apply_property_sequence(
    nfc_device *pnd,
    const property_bool_setting_t *settings,
    size_t count)
{
  for (size_t i = 0; i < count; i++)
  {
    const int res = nfc_device_set_property_bool(
        pnd,
        settings[i].property,
        settings[i].value);
    if (res < 0)
      return res;
  }
  return NFC_SUCCESS;
}

int nfc_device_set_property_int(
    nfc_device *pnd,
    const nfc_property property,
    const int value)
{
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "set_property_int %s %s",
          nfc_property_name[property],
          value ? "True" : "False");
  return HAL(device_set_property_int, pnd, property, value);
}

int nfc_device_set_property_bool(
    nfc_device *pnd,
    const nfc_property property,
    const bool bEnable)
{
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "set_property_bool %s %s",
          nfc_property_name[property],
          bEnable ? "True" : "False");
  return HAL(device_set_property_bool, pnd, property, bEnable);
}

/* ============================================================================
 * MODULATION VALIDATION
 * ========================================================================== */

static inline bool
modulation_supported(
    const nfc_modulation_type *supported,
    const nfc_modulation_type value)
{
  if (!supported)
    return false;

  for (int i = 0; supported[i]; i++)
  {
    if (supported[i] == value)
    {
      return true;
    }
  }
  return false;
}

static inline bool
baud_rate_supported(
    const nfc_baud_rate *rates,
    const nfc_baud_rate value)
{
  if (!rates)
    return false;

  for (int i = 0; rates[i]; i++)
  {
    if (rates[i] == value)
    {
      return true;
    }
  }
  return false;
}

static const nfc_baud_rate *
get_baud_rates_for_mode(
    nfc_device *pnd,
    const nfc_mode mode,
    const nfc_modulation_type type,
    int *status)
{
  const nfc_baud_rate *rates = NULL;

  if (mode == N_INITIATOR)
  {
    *status = nfc_device_get_supported_baud_rate(pnd, type, &rates);
  }
  else
  {
    *status = nfc_device_get_supported_baud_rate_target_mode(pnd, type, &rates);
  }

  return (*status < 0) ? NULL : rates;
}

static int
nfc_device_validate_modulation(
    nfc_device *pnd,
    const nfc_mode mode,
    const nfc_modulation *nm)
{
  const nfc_modulation_type *supported_types = NULL;
  int res = nfc_device_get_supported_modulation(pnd, mode, &supported_types);
  if (res < 0)
    return res;

  if (!modulation_supported(supported_types, nm->nmt))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "Modulation type not supported");
    return NFC_EINVARG;
  }

  const nfc_baud_rate *supported_rates =
      get_baud_rates_for_mode(pnd, mode, nm->nmt, &res);
  if (res < 0)
    return res;

  if (baud_rate_supported(supported_rates, nm->nbr))
  {
    return NFC_SUCCESS;
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "Baud rate not supported");
  return NFC_EINVARG;
}

/* ============================================================================
 * INITIATOR MODE FUNCTIONS
 * ========================================================================== */

int nfc_initiator_init(nfc_device *pnd)
{
  static const property_bool_setting_t INITIATOR_SETTINGS[] = {
      {NP_ACTIVATE_FIELD, false},
      {NP_ACTIVATE_FIELD, true},
      {NP_INFINITE_SELECT, true},
      {NP_AUTO_ISO14443_4, true},
      {NP_FORCE_ISO14443_A, true},
      {NP_FORCE_SPEED_106, true},
      {NP_ACCEPT_INVALID_FRAMES, false},
      {NP_ACCEPT_MULTIPLE_FRAMES, false}};

  const int res = apply_property_sequence(
      pnd,
      INITIATOR_SETTINGS,
      sizeof(INITIATOR_SETTINGS) / sizeof(INITIATOR_SETTINGS[0]));

  if (res < 0)
    return res;

  return HAL(initiator_init, pnd);
}

int nfc_initiator_init_secure_element(nfc_device *pnd)
{
  return HAL(initiator_init_secure_element, pnd);
}

/* Continues with remaining initiator functions... */

/* ============================================================================
 * TARGET MODE FUNCTIONS
 * ========================================================================== */

int nfc_target_init(
    nfc_device *pnd,
    nfc_target *pnt,
    uint8_t *pbtRx,
    const size_t szRx,
    int timeout)
{
  static const property_bool_setting_t TARGET_SETTINGS[] = {
      {NP_ACCEPT_INVALID_FRAMES, false},
      {NP_ACCEPT_MULTIPLE_FRAMES, false},
      {NP_HANDLE_CRC, true},
      {NP_HANDLE_PARITY, true},
      {NP_AUTO_ISO14443_4, true},
      {NP_EASY_FRAMING, true},
      {NP_ACTIVATE_CRYPTO1, false},
      {NP_ACTIVATE_FIELD, false}};

  const int res = apply_property_sequence(
      pnd,
      TARGET_SETTINGS,
      sizeof(TARGET_SETTINGS) / sizeof(TARGET_SETTINGS[0]));

  if (res < 0)
    return res;

  return HAL(target_init, pnd, pnt, pbtRx, szRx, timeout);
}

/* ============================================================================
 * ERROR HANDLING
 * ========================================================================== */

const char *
nfc_strerror(const nfc_device *pnd)
{
  for (size_t i = 0; i < sizeof(ERROR_MESSAGES) / sizeof(ERROR_MESSAGES[0]); i++)
  {
    if (ERROR_MESSAGES[i].error_code == pnd->last_error)
    {
      return ERROR_MESSAGES[i].error_msg;
    }
  }
  return "Unknown error";
}

int nfc_strerror_r(const nfc_device *pnd, char *pcStrErrBuf, size_t szBufLen)
{
  return (snprintf(pcStrErrBuf, szBufLen, "%s", nfc_strerror(pnd)) < 0) ? -1 : 0;
}

void nfc_perror(const nfc_device *pnd, const char *pcString)
{
  fprintf(stderr, "%s: %s\n", pcString, nfc_strerror(pnd));
}

int nfc_device_get_last_error(const nfc_device *pnd)
{
  return pnd->last_error;
}

/* ============================================================================
 * STRING CONVERSION UTILITIES
 * ========================================================================== */

static const char *
lookup_modulation_type_name(const nfc_modulation_type type)
{
  for (size_t i = 0;
       i < sizeof(MODULATION_TYPE_NAMES) / sizeof(MODULATION_TYPE_NAMES[0]);
       i++)
  {
    if (MODULATION_TYPE_NAMES[i].type == type)
    {
      return MODULATION_TYPE_NAMES[i].name;
    }
  }
  return "???";
}

const char *
str_nfc_baud_rate(const nfc_baud_rate nbr)
{
  switch (nbr)
  {
  case NBR_UNDEFINED:
    return "undefined baud rate";
  case NBR_106:
    return "106 kbps";
  case NBR_212:
    return "212 kbps";
  case NBR_424:
    return "424 kbps";
  case NBR_847:
    return "847 kbps";
  }
  return "???";
}

const char *
str_nfc_modulation_type(const nfc_modulation_type nmt)
{
  return lookup_modulation_type_name(nmt);
}

int str_nfc_target(char **buf, const nfc_target *pnt, bool verbose)
{
  *buf = malloc(4096);
  if (!*buf)
    return NFC_ESOFT;

  (*buf)[0] = '\0';
  snprint_nfc_target(*buf, 4096, pnt, verbose);
  return (int)nfc_safe_strlen(*buf, 4096);
}

/* ============================================================================
 * TARGET SELECTION AND LISTING
 * ========================================================================== */

static inline bool
target_already_seen(const nfc_target *targets, size_t count, const nfc_target *candidate)
{
  for (size_t i = 0; i < count; i++)
  {
    if (memcmp(&targets[i], candidate, sizeof(nfc_target)) == 0)
    {
      return true;
    }
  }
  return false;
}

static inline bool
modulation_requires_single_attempt(const nfc_modulation nm)
{
  switch (nm.nmt)
  {
  case NMT_FELICA:
  case NMT_JEWEL:
  case NMT_BARCODE:
  case NMT_ISO14443BI:
  case NMT_ISO14443B2SR:
  case NMT_ISO14443B2CT:
    return true;
  default:
    return false;
  }
}

int nfc_initiator_select_passive_target(
    nfc_device *pnd,
    const nfc_modulation nm,
    const uint8_t *pbtInitData,
    const size_t szInitData,
    nfc_target *pnt)
{
  int res = nfc_device_validate_modulation(pnd, N_INITIATOR, &nm);
  if (res != NFC_SUCCESS)
  {
    return res;
  }

  uint8_t *abtInit = NULL;
  size_t szInit = 0;

  if (szInitData == 0)
  {
    /* Provide default values if needed */
    prepare_initiator_data(nm, &abtInit, &szInit);
    return HAL(initiator_select_passive_target, pnd, nm, abtInit, szInit, pnt);
  }

  const size_t maxAbt = (szInitData > 12) ? szInitData : 12;
  abtInit = malloc(sizeof(uint8_t) * maxAbt);
  if (!abtInit)
  {
    return NFC_ESOFT;
  }

  if (nm.nmt == NMT_ISO14443A)
  {
    iso14443_cascade_uid(pbtInitData, szInitData, abtInit, &szInit);
  }
  else
  {
    if (nfc_safe_memcpy(abtInit, maxAbt, pbtInitData, szInitData) < 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
              "Failed to copy init data");
      free(abtInit);
      return NFC_EINVARG;
    }
    szInit = szInitData;
  }

  res = HAL(initiator_select_passive_target, pnd, nm, abtInit, szInit, pnt);
  free(abtInit);
  return res;
}

int nfc_initiator_list_passive_targets(
    nfc_device *pnd,
    const nfc_modulation nm,
    nfc_target ant[],
    const size_t szTargets)
{
  if (szTargets == 0)
    return 0;

  uint8_t *pbtInitData = NULL;
  size_t szInitDataLen = 0;
  size_t target_count = 0;
  int res = 0;

  pnd->last_error = 0;

  const bool restore_infinite = pnd->bInfiniteSelect;
  if ((res = nfc_device_set_property_bool(pnd, NP_INFINITE_SELECT, false)) < 0)
  {
    return res;
  }

  prepare_initiator_data(nm, &pbtInitData, &szInitDataLen);

  nfc_target nt;
  while (nfc_initiator_select_passive_target(pnd, nm, pbtInitData, szInitDataLen, &nt) > 0)
  {
    if (target_already_seen(ant, target_count, &nt))
    {
      break;
    }

    if (nfc_safe_memcpy(&ant[target_count], sizeof(nfc_target),
                        &nt, sizeof(nfc_target)) < 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
              "Failed to copy target data");
      res = NFC_EIO;
      break;
    }

    target_count++;

    if (target_count >= szTargets || modulation_requires_single_attempt(nm))
    {
      break;
    }

    nfc_initiator_deselect_target(pnd);
  }

  if (restore_infinite)
  {
    const int restore_res = nfc_device_set_property_bool(pnd, NP_INFINITE_SELECT, true);
    if (restore_res < 0 && res >= 0)
    {
      res = restore_res;
    }
  }

  return (res < 0) ? res : (int)target_count;
}

int nfc_initiator_poll_target(
    nfc_device *pnd,
    const nfc_modulation *pnmModulations,
    const size_t szModulations,
    const uint8_t uiPollNr,
    const uint8_t uiPeriod,
    nfc_target *pnt)
{
  return HAL(initiator_poll_target, pnd, pnmModulations, szModulations,
             uiPollNr, uiPeriod, pnt);
}

int nfc_initiator_deselect_target(nfc_device *pnd)
{
  return HAL(initiator_deselect_target, pnd);
}

/* ============================================================================
 * D.E.P. (DATA EXCHANGE PROTOCOL) FUNCTIONS
 * ========================================================================== */

int nfc_initiator_select_dep_target(
    nfc_device *pnd,
    const nfc_dep_mode ndm,
    const nfc_baud_rate nbr,
    const nfc_dep_info *pndiInitiator,
    nfc_target *pnt,
    const int timeout)
{
  return HAL(initiator_select_dep_target, pnd, ndm, nbr, pndiInitiator, pnt, timeout);
}

int nfc_initiator_poll_dep_target(
    struct nfc_device *pnd,
    const nfc_dep_mode ndm,
    const nfc_baud_rate nbr,
    const nfc_dep_info *pndiInitiator,
    nfc_target *pnt,
    const int timeout)
{
  const int period = 300;
  int remaining_time = timeout;
  int res;
  int result = 0;

  const bool bInfiniteSelect = pnd->bInfiniteSelect;
  if ((res = nfc_device_set_property_bool(pnd, NP_INFINITE_SELECT, true)) < 0)
  {
    return res;
  }

  while (remaining_time > 0)
  {
    res = nfc_initiator_select_dep_target(pnd, ndm, nbr, pndiInitiator, pnt, period);

    if (res < 0 && res != NFC_ETIMEOUT)
    {
      result = res;
      goto end;
    }

    if (res == 1)
    {
      result = res;
      goto end;
    }

    remaining_time -= period;
  }

end:
  if (!bInfiniteSelect)
  {
    if ((res = nfc_device_set_property_bool(pnd, NP_INFINITE_SELECT, false)) < 0)
    {
      return res;
    }
  }
  return result;
}

/* ============================================================================
 * DATA TRANSMISSION FUNCTIONS
 * ========================================================================== */

int nfc_initiator_transceive_bytes(
    nfc_device *pnd,
    const uint8_t *pbtTx,
    const size_t szTx,
    uint8_t *pbtRx,
    const size_t szRx,
    int timeout)
{
  return HAL(initiator_transceive_bytes, pnd, pbtTx, szTx, pbtRx, szRx, timeout);
}

int nfc_initiator_transceive_bits(
    nfc_device *pnd,
    const uint8_t *pbtTx,
    const size_t szTxBits,
    const uint8_t *pbtTxPar,
    uint8_t *pbtRx,
    const size_t szRx,
    uint8_t *pbtRxPar)
{
  (void)szRx;
  return HAL(initiator_transceive_bits, pnd, pbtTx, szTxBits, pbtTxPar, pbtRx, pbtRxPar);
}

int nfc_initiator_transceive_bytes_timed(
    nfc_device *pnd,
    const uint8_t *pbtTx,
    const size_t szTx,
    uint8_t *pbtRx,
    const size_t szRx,
    uint32_t *cycles)
{
  return HAL(initiator_transceive_bytes_timed, pnd, pbtTx, szTx, pbtRx, szRx, cycles);
}

int nfc_initiator_transceive_bits_timed(
    nfc_device *pnd,
    const uint8_t *pbtTx,
    const size_t szTxBits,
    const uint8_t *pbtTxPar,
    uint8_t *pbtRx,
    const size_t szRx,
    uint8_t *pbtRxPar,
    uint32_t *cycles)
{
  (void)szRx;
  return HAL(initiator_transceive_bits_timed, pnd, pbtTx, szTxBits, pbtTxPar,
             pbtRx, pbtRxPar, cycles);
}

int nfc_initiator_target_is_present(nfc_device *pnd, const nfc_target *pnt)
{
  return HAL(initiator_target_is_present, pnd, pnt);
}

/* ============================================================================
 * TARGET MODE DATA TRANSMISSION
 * ========================================================================== */

int nfc_target_send_bytes(
    nfc_device *pnd,
    const uint8_t *pbtTx,
    const size_t szTx,
    int timeout)
{
  return HAL(target_send_bytes, pnd, pbtTx, szTx, timeout);
}

int nfc_target_receive_bytes(
    nfc_device *pnd,
    uint8_t *pbtRx,
    const size_t szRx,
    int timeout)
{
  return HAL(target_receive_bytes, pnd, pbtRx, szRx, timeout);
}

int nfc_target_send_bits(
    nfc_device *pnd,
    const uint8_t *pbtTx,
    const size_t szTxBits,
    const uint8_t *pbtTxPar)
{
  return HAL(target_send_bits, pnd, pbtTx, szTxBits, pbtTxPar);
}

int nfc_target_receive_bits(
    nfc_device *pnd,
    uint8_t *pbtRx,
    const size_t szRx,
    uint8_t *pbtRxPar)
{
  return HAL(target_receive_bits, pnd, pbtRx, szRx, pbtRxPar);
}

/* ============================================================================
 * DEVICE CONTROL FUNCTIONS
 * ========================================================================== */

void nfc_close(nfc_device *pnd)
{
  if (pnd)
  {
    pnd->driver->close(pnd);
  }
}

int nfc_idle(nfc_device *pnd)
{
  return HAL(idle, pnd);
}

int nfc_abort_command(nfc_device *pnd)
{
  return HAL(abort_command, pnd);
}

/* ============================================================================
 * LIBRARY INITIALIZATION AND CLEANUP
 * ========================================================================== */

void nfc_init(nfc_context **context)
{
  *context = nfc_context_new();
  if (!*context)
  {
    perror("malloc");
    return;
  }

  if (!nfc_drivers)
  {
    nfc_drivers_init();
  }
}

void nfc_exit(nfc_context *context)
{
  while (nfc_drivers)
  {
    struct nfc_driver_list *pndl = (struct nfc_driver_list *)nfc_drivers;
    nfc_drivers = pndl->next;
    free(pndl);
  }

  nfc_context_free(context);
}

/* ============================================================================
 * DEVICE INFORMATION AND DATA ACCESSORS
 * ========================================================================== */

const char *
nfc_device_get_name(nfc_device *pnd)
{
  return pnd->name;
}

const char *
nfc_device_get_connstring(nfc_device *pnd)
{
  return pnd->connstring;
}

int nfc_device_get_supported_modulation(
    nfc_device *pnd,
    const nfc_mode mode,
    const nfc_modulation_type **const supported_mt)
{
  return HAL(get_supported_modulation, pnd, mode, supported_mt);
}

int nfc_device_get_supported_baud_rate(
    nfc_device *pnd,
    const nfc_modulation_type nmt,
    const nfc_baud_rate **const supported_br)
{
  return HAL(get_supported_baud_rate, pnd, N_INITIATOR, nmt, supported_br);
}

int nfc_device_get_supported_baud_rate_target_mode(
    nfc_device *pnd,
    const nfc_modulation_type nmt,
    const nfc_baud_rate **const supported_br)
{
  return HAL(get_supported_baud_rate, pnd, N_TARGET, nmt, supported_br);
}

int nfc_device_get_information_about(nfc_device *pnd, char **buf)
{
  return HAL(device_get_information_about, pnd, buf);
}

/* ============================================================================
 * MISCELLANEOUS FUNCTIONS
 * ========================================================================== */

const char *
nfc_version(void)
{
#ifdef GIT_REVISION
  return GIT_REVISION;
#else
  return PACKAGE_VERSION;
#endif
}

void nfc_free(void *p)
{
  free(p);
}
