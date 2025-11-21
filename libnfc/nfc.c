/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartiere
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 * Copyright (C) 2020      Adam Laurie
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
 * @file nfc.c
 * @brief NFC library implementation
 */
/**
 * @defgroup lib Library initialization/deinitialization
 * This page details how to initialize and deinitialize libnfc. Initialization
 * must be performed before using any libnfc functionality, and similarly you
 * must not call any libnfc functions after deinitialization.
 */
/**
 * @defgroup dev NFC Device/Hardware manipulation
 * The functionality documented below is designed to help with the following
 * operations:
 * - Enumerating the NFC devices currently attached to the system
 * - Opening and closing the chosen device
 */
/**
 * @defgroup initiator  NFC initiator
 * This page details how to act as "reader".
 */
/**
 * @defgroup target  NFC target
 * This page details how to act as tag (i.e. MIFARE Classic) or NFC target device.
 */
/**
 * @defgroup error  Error reporting
 * Most libnfc functions return 0 on success or one of error codes defined on failure.
 */
/**
 * @defgroup data  Special data accessors
 * The functionnality documented below allow to access to special data as device name or device connstring.
 */
/**
 * @defgroup properties  Properties accessors
 * The functionnality documented below allow to configure parameters and registers.
 */
/**
 * @defgroup misc Miscellaneous
 *
 */
/**
 * @defgroup string-converter  To-string converters
 * The functionnality documented below allow to retreive some information in text format.
 */

#ifdef HAVE_CONFIG_H
#include "config.h"
#endif // HAVE_CONFIG_H

#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
#include <assert.h>
#include <ctype.h>

// Declare strnlen if not available (MSVC 2015+ already provides it)
#if !defined(HAVE_STRNLEN)
#  if !(defined(_MSC_VER) && _MSC_VER >= 1900)
extern size_t strnlen(const char *s, size_t maxlen);
#  endif
#endif

#include <nfc/nfc.h>

#include "nfc-internal.h"
#include "nfc-secure.h"
#include "target-subr.h"
#include "drivers.h"

#ifndef PACKAGE_VERSION
#define PACKAGE_VERSION "unknown"
#endif

#if defined(DRIVER_PCSC_ENABLED)
#include "drivers/pcsc.h"
#endif /* DRIVER_PCSC_ENABLED */

#if defined(DRIVER_ACR122_PCSC_ENABLED)
#include "drivers/acr122_pcsc.h"
#endif /* DRIVER_ACR122_PCSC_ENABLED */

#if defined(DRIVER_ACR122_USB_ENABLED)
#include "drivers/acr122_usb.h"
#endif /* DRIVER_ACR122_USB_ENABLED */

#if defined(DRIVER_ACR122S_ENABLED)
#include "drivers/acr122s.h"
#endif /* DRIVER_ACR122S_ENABLED */

#if defined(DRIVER_PN53X_USB_ENABLED)
#include "drivers/pn53x_usb.h"
#endif /* DRIVER_PN53X_USB_ENABLED */

#if defined(DRIVER_ARYGON_ENABLED)
#include "drivers/arygon.h"
#endif /* DRIVER_ARYGON_ENABLED */

#if defined(DRIVER_PN532_UART_ENABLED)
#include "drivers/pn532_uart.h"
#endif /* DRIVER_PN532_UART_ENABLED */

#if defined(DRIVER_PN532_SPI_ENABLED)
#include "drivers/pn532_spi.h"
#endif /* DRIVER_PN532_SPI_ENABLED */

#if defined(DRIVER_PN532_I2C_ENABLED)
#include "drivers/pn532_i2c.h"
#endif /* DRIVER_PN532_I2C_ENABLED */

#if defined(DRIVER_PN71XX_ENABLED)
#include "drivers/pn71xx.h"
#endif /* DRIVER_PN71XX_ENABLED */

#define LOG_CATEGORY "libnfc.general"
#define LOG_GROUP NFC_LOG_GROUP_GENERAL

#define NFC_DRIVER_NAME_MAX 64

struct nfc_driver_list
{
  const struct nfc_driver_list *next;
  const struct nfc_driver *driver;
};

const struct nfc_driver_list *nfc_drivers = NULL;

// descritions for debugging
const char *nfc_property_name[] = {
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

static void
nfc_drivers_init(void)
{
#if defined(DRIVER_PN53X_USB_ENABLED)
  nfc_register_driver(&pn53x_usb_driver);
#endif /* DRIVER_PN53X_USB_ENABLED */
#if defined(DRIVER_PCSC_ENABLED)
  nfc_register_driver(&pcsc_driver);
#endif /* DRIVER_PCSC_ENABLED */
  /* Removed drivers (files deleted):
   * - acr122_pcsc (use pcsc driver instead)
   * - acr122_usb (use pn53x_usb driver instead)
   * - acr122s (serial version, rarely used)
   */
#if defined(DRIVER_PN532_UART_ENABLED)
  nfc_register_driver(&pn532_uart_driver);
#endif /* DRIVER_PN532_UART_ENABLED */
#if defined(DRIVER_PN532_SPI_ENABLED)
  nfc_register_driver(&pn532_spi_driver);
#endif /* DRIVER_PN532_SPI_ENABLED */
#if defined(DRIVER_PN532_I2C_ENABLED)
  nfc_register_driver(&pn532_i2c_driver);
#endif /* DRIVER_PN532_I2C_ENABLED */
#if defined(DRIVER_ARYGON_ENABLED)
  nfc_register_driver(&arygon_driver);
#endif /* DRIVER_ARYGON_ENABLED */
  /* Removed driver:
   * - pn71xx (experimental NCI driver, deleted file)
   */
}

static int
nfc_device_validate_modulation(nfc_device *pnd, const nfc_mode mode, const nfc_modulation *nm);

/** @ingroup lib
 * @brief Register an NFC device driver with libnfc.
 * This function registers a driver with libnfc, the caller is responsible of managing the lifetime of the
 * driver and make sure that any resources associated with the driver are available after registration.
 * @param pnd Pointer to an NFC device driver to be registered.
 * @retval NFC_SUCCESS If the driver registration succeeds.
 */
int nfc_register_driver(const struct nfc_driver *ndr)
{
  if (!ndr)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "nfc_register_driver returning NFC_EINVARG");
    return NFC_EINVARG;
  }

  struct nfc_driver_list *pndl = (struct nfc_driver_list *)malloc(sizeof(struct nfc_driver_list));
  if (!pndl)
    return NFC_ESOFT;

  pndl->driver = ndr;
  pndl->next = nfc_drivers;
  nfc_drivers = pndl;

  return NFC_SUCCESS;
}

/** @ingroup lib
 * @brief Initialize libnfc.
 * This function must be called before calling any other libnfc function
 * @param context Output location for nfc_context
 */
void nfc_init(nfc_context **context)
{
  *context = nfc_context_new();
  if (!*context)
  {
    perror("malloc");
    return;
  }
  if (!nfc_drivers)
    nfc_drivers_init();
}

/** @ingroup lib
 * @brief Deinitialize libnfc.
 * Should be called after closing all open devices and before your application terminates.
 * @param context The context to deinitialize
 */
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

static bool
contains_control_characters(const char *value, size_t length)
{
  if (!value)
    return false;

  for (size_t i = 0; i < length && value[i]; i++)
  {
    if (!isprint((unsigned char)value[i]))
      return true;
  }

  return false;
}

static bool
copy_connstring_safely(const char *source, nfc_connstring destination)
{
  if (!source)
    return false;

  size_t length = nfc_safe_strlen(source, NFC_BUFSIZE_CONNSTRING);
  if (contains_control_characters(source, length))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Connection string contains control characters");
    return false;
  }

  if (length >= NFC_BUFSIZE_CONNSTRING)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Connection string exceeds maximum length");
    return false;
  }

  if (nfc_safe_memcpy(destination, NFC_BUFSIZE_CONNSTRING, source, length + 1) < 0)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to copy connection string");
    return false;
  }

  destination[length] = '\0';
  return true;
}

static bool
connstring_is_usb_request(const nfc_connstring ncs)
{
  return strncmp(ncs, "usb", 3) == 0;
}

static bool
prepare_connstring(nfc_context *context, const nfc_connstring connstring, nfc_connstring destination)
{
  if (!connstring)
  {
    nfc_connstring discovered;
    if (nfc_list_devices(context, &discovered, 1) == 0)
      return false;

    return copy_connstring_safely(discovered, destination);
  }

  return copy_connstring_safely(connstring, destination);
}

static bool
driver_matches_connstring(const struct nfc_driver *ndr, const nfc_connstring ncs, bool request_is_usb)
{
  if (!ndr || !ndr->name)
    return false;

  size_t name_len = nfc_safe_strlen(ndr->name, NFC_DRIVER_NAME_MAX);
  if (strncmp(ndr->name, ncs, name_len) == 0)
    return true;

  if (!request_is_usb || name_len < 4)
    return false;

  return strncmp(ndr->name + (name_len - 4), "_usb", 4) == 0;
}

static bool
apply_user_defined_device_name(nfc_context *context, const nfc_connstring ncs, nfc_device *pnd)
{
  if (!context || !pnd)
    return true;

  for (uint32_t i = 0; i < context->user_defined_device_count; i++)
  {
    if (strcmp(ncs, context->user_defined_devices[i].connstring) != 0)
      continue;

    size_t name_len = nfc_safe_strlen(context->user_defined_devices[i].name, DEVICE_NAME_LENGTH);
    if (nfc_safe_memcpy(pnd->name, DEVICE_NAME_LENGTH, context->user_defined_devices[i].name, name_len) < 0)
      return false;
    pnd->name[name_len] = '\0';
    break;
  }
  return true;
}

typedef enum
{
  NFC_DRIVER_SKIP,
  NFC_DRIVER_OPENED,
  NFC_DRIVER_ABORT
} nfc_driver_open_result;

static nfc_driver_open_result
attempt_open_driver(nfc_context *context, const nfc_connstring ncs, bool request_is_usb,
                    const struct nfc_driver *ndr, nfc_device **out_device)
{
  if (!driver_matches_connstring(ndr, ncs, request_is_usb))
    return NFC_DRIVER_SKIP;

  nfc_device *candidate = ndr->open(context, ncs);
  if (!candidate)
  {
    if (request_is_usb)
      return NFC_DRIVER_SKIP;

    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "Unable to open \"%s\".", ncs);
    return NFC_DRIVER_ABORT;
  }

  *out_device = candidate;
  return NFC_DRIVER_OPENED;
}

static bool
finalize_opened_device(nfc_context *context, const nfc_connstring ncs, nfc_device *pnd)
{
  if (!apply_user_defined_device_name(context, ncs, pnd))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to copy device name");
    nfc_close(pnd);
    return false;
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "\"%s\" (%s) has been claimed.", pnd->name, pnd->connstring);
  return true;
}

#ifdef CONFFILES
static bool
copy_connstring_entry(nfc_connstring connstrings[], size_t index, const nfc_connstring source)
{
  if (!copy_connstring_safely(source, connstrings[index]))
    return false;

  return true;
}

#ifdef ENVVARS
static bool
string_is_numeric(const char *value, size_t length)
{
  if (!value || length == 0)
    return false;

  for (size_t i = 0; i < length && value[i]; i++)
  {
    if (!isdigit((unsigned char)value[i]))
      return false;
  }
  return true;
}

static char *
duplicate_log_level_env(void)
{
  char *copy = NULL;
  char *env_log_level = getenv("LIBNFC_LOG_LEVEL");
  if (!env_log_level)
    return NULL;

  size_t env_len = nfc_safe_strlen(env_log_level, 256);
  if (env_len >= 256)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN, "LIBNFC_LOG_LEVEL value is too long");
    return NULL;
  }
  if (!string_is_numeric(env_log_level, env_len) || contains_control_characters(env_log_level, env_len))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN, "Ignoring invalid LIBNFC_LOG_LEVEL value");
    return NULL;
  }

  copy = malloc(env_len + 1);
  if (!copy)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "%s", "Unable to malloc()");
    return NULL;
  }

  if (nfc_safe_memcpy(copy, env_len + 1, env_log_level, env_len) < 0)
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to copy log level");
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
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN, "Unable to restore LIBNFC_LOG_LEVEL");
    free(old_value);
  }
  else if (!had_env)
  {
    if (unsetenv("LIBNFC_LOG_LEVEL") != 0)
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN, "Unable to unset LIBNFC_LOG_LEVEL");
  }
}
#endif // ENVVARS

static bool
optional_device_available(nfc_context *context, const struct nfc_user_defined_device *device)
{
#ifdef ENVVARS
  const char *current_log_level = getenv("LIBNFC_LOG_LEVEL");
  bool had_env = current_log_level != NULL;
  char *old_env_log_level = duplicate_log_level_env();
  if (!had_env || old_env_log_level)
  {
    if (setenv("LIBNFC_LOG_LEVEL", "0", 1) != 0)
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN, "Unable to reduce log verbosity when probing optional device");
  }
#endif

  nfc_device *pnd = nfc_open(context, device->connstring);

#ifdef ENVVARS
  restore_log_level_env(old_env_log_level, had_env);
#endif

  if (!pnd)
    return false;

  nfc_close(pnd);
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "User device %s found", device->name);
  return true;
}

static size_t
append_user_defined_devices(nfc_context *context, nfc_connstring connstrings[], const size_t connstrings_len)
{
  size_t device_found = 0;
  for (uint32_t i = 0; i < context->user_defined_device_count && device_found < connstrings_len; i++)
  {
    const struct nfc_user_defined_device *device = &context->user_defined_devices[i];
    if (device->optional && !optional_device_available(context, device))
      continue;

    if (!copy_connstring_entry(connstrings, device_found, device->connstring))
      continue;

    device_found++;
  }
  return device_found;
}
#endif // CONFFILES

static bool
scan_allowed_for_driver(const nfc_context *context, const struct nfc_driver *ndr)
{
  if (!ndr)
    return false;

  return (ndr->scan_type == NOT_INTRUSIVE) || (context->allow_intrusive_scan && (ndr->scan_type == INTRUSIVE));
}

static size_t
autoscan_devices(nfc_context *context, nfc_connstring connstrings[], size_t start_index, const size_t connstrings_len)
{
  size_t device_found = start_index;
  for (const struct nfc_driver_list *pndl = nfc_drivers; pndl && device_found < connstrings_len; pndl = pndl->next)
  {
    const struct nfc_driver *ndr = pndl->driver;
    if (!scan_allowed_for_driver(context, ndr))
      continue;

    size_t remaining = connstrings_len - device_found;
    size_t newly_found = ndr->scan(context, connstrings + device_found, remaining);
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "%ld device(s) found using %s driver", (unsigned long)newly_found, ndr->name);
    if (newly_found > 0)
      device_found += newly_found;
  }
  return device_found;
}

/** @ingroup dev
 * @brief Open a NFC device
 * @param context The context to operate on.
 * @param connstring The device connection string if specific device is wanted, \c NULL otherwise
 * @return Returns pointer to a \a nfc_device struct if successfull; otherwise returns \c NULL value.
 *
 * If \e connstring is \c NULL, the first available device from \a nfc_list_devices function is used.
 *
 * If \e connstring is set, this function will try to claim the right device using information provided by \e connstring.
 *
 * When it has successfully claimed a NFC device, memory is allocated to save the device information.
 * It will return a pointer to a \a nfc_device struct.
 * This pointer should be supplied by every next functions of libnfc that should perform an action with this device.
 *
 * @note Depending on the desired operation mode, the device needs to be configured by using nfc_initiator_init() or nfc_target_init(),
 * optionally followed by manual tuning of the parameters if the default parameters are not suiting your goals.
 */
nfc_device *
nfc_open(nfc_context *context, const nfc_connstring connstring)
{
  nfc_connstring ncs;
  if (!prepare_connstring(context, connstring, ncs))
    return NULL;

  const bool request_is_usb = connstring_is_usb_request(ncs);
  for (const struct nfc_driver_list *pndl = nfc_drivers; pndl; pndl = pndl->next)
  {
    nfc_device *candidate = NULL;
    nfc_driver_open_result result = attempt_open_driver(context, ncs, request_is_usb, pndl->driver, &candidate);

    if (result == NFC_DRIVER_SKIP)
      continue;

    if (result == NFC_DRIVER_ABORT)
      return NULL;

    if (candidate && finalize_opened_device(context, ncs, candidate))
      return candidate;

    return NULL;
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "No driver available to handle \"%s\".", ncs);
  return NULL;
}

/** @ingroup dev
 * @brief Close from a NFC device
 * @param pnd \a nfc_device struct pointer that represent currently used device
 *
 * Initiator's selected tag is closed and the device, including allocated \a nfc_device struct, is released.
 */
void nfc_close(nfc_device *pnd)
{
  if (pnd)
  {
    // Close, clean up and release the device
    pnd->driver->close(pnd);
  }
}

/** @ingroup dev
 * @brief Scan for discoverable supported devices (ie. only available for some drivers)
 * @return Returns the number of devices found.
 * @param context The context to operate on, or NULL for the default context.
 * @param connstrings array of \a nfc_connstring.
 * @param connstrings_len size of the \a connstrings array.
 *
 */
size_t
nfc_list_devices(nfc_context *context, nfc_connstring connstrings[], const size_t connstrings_len)
{
  if (!context || !connstrings || connstrings_len == 0)
    return 0;

  size_t device_found = 0;

#ifdef CONFFILES
  device_found = append_user_defined_devices(context, connstrings, connstrings_len);
  if (device_found >= connstrings_len)
    return device_found;
#endif // CONFFILES

  if (!context->allow_autoscan)
  {
    if (context->user_defined_device_count == 0)
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, "Warning: %s", "user must specify device(s) manually when autoscan is disabled");
    return device_found;
  }

  device_found = autoscan_devices(context, connstrings, device_found, connstrings_len);
  return device_found;
}

/** @ingroup properties
 * @brief Set a device's integer-property value
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param property \a nfc_property which will be set
 * @param value integer value
 *
 * Sets integer property.
 *
 * @see nfc_property enum values
 */
int nfc_device_set_property_int(nfc_device *pnd, const nfc_property property, const int value)
{
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "set_property_int %s %s", nfc_property_name[property], value ? "True" : "False");
  return HAL(device_set_property_int, pnd, property, value);
}

/** @ingroup properties
 * @brief Set a device's boolean-property value
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param property \a nfc_property which will be set
 * @param bEnable boolean to activate/disactivate the property
 *
 * Configures parameters and registers that control for example timing,
 * modulation, frame and error handling.  There are different categories for
 * configuring the \e PN53X chip features (handle, activate, infinite and
 * accept).
 */
int nfc_device_set_property_bool(nfc_device *pnd, const nfc_property property, const bool bEnable)
{
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "set_property_bool %s %s", nfc_property_name[property], bEnable ? "True" : "False");
  return HAL(device_set_property_bool, pnd, property, bEnable);
}

struct property_bool_setting
{
  nfc_property property;
  bool value;
};

static int
apply_property_sequence(nfc_device *pnd, const struct property_bool_setting *settings, size_t count)
{
  for (size_t i = 0; i < count; i++)
  {
    int res = nfc_device_set_property_bool(pnd, settings[i].property, settings[i].value);
    if (res < 0)
      return res;
  }
  return NFC_SUCCESS;
}

static bool
target_already_seen(const nfc_target *targets, size_t count, const nfc_target *candidate)
{
  for (size_t i = 0; i < count; i++)
  {
    if (memcmp(&targets[i], candidate, sizeof(nfc_target)) == 0)
      return true;
  }
  return false;
}

static bool
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

static bool
modulation_supported(const nfc_modulation_type *supported, const nfc_modulation_type value)
{
  if (!supported)
    return false;

  for (int i = 0; supported[i]; i++)
  {
    if (supported[i] == value)
      return true;
  }
  return false;
}

static const nfc_baud_rate *
get_baud_rates_for_mode(nfc_device *pnd, const nfc_mode mode, const nfc_modulation_type type, int *status)
{
  const nfc_baud_rate *rates = NULL;
  if (mode == N_INITIATOR)
    *status = nfc_device_get_supported_baud_rate(pnd, type, &rates);
  else
    *status = nfc_device_get_supported_baud_rate_target_mode(pnd, type, &rates);

  return (*status < 0) ? NULL : rates;
}

static bool
baud_rate_supported(const nfc_baud_rate *rates, const nfc_baud_rate value)
{
  if (!rates)
    return false;

  for (int i = 0; rates[i]; i++)
  {
    if (rates[i] == value)
      return true;
  }
  return false;
}

static const char *
lookup_modulation_type_name(const nfc_modulation_type type)
{
  static const struct
  {
    nfc_modulation_type type;
    const char *name;
  } mapping[] = {
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

  for (size_t i = 0; i < sizeof(mapping) / sizeof(mapping[0]); i++)
  {
    if (mapping[i].type == type)
      return mapping[i].name;
  }

  return "???";
}

/** @ingroup initiator
 * @brief Initialize NFC device as initiator (reader)
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 *
 * The NFC device is configured to function as RFID reader.
 * After initialization it can be used to communicate to passive RFID tags and active NFC devices.
 * The reader will act as initiator to communicate peer 2 peer (NFCIP) to other active NFC devices.
 * - Crc is handled by the device (NP_HANDLE_CRC = true)
 * - Parity is handled the device (NP_HANDLE_PARITY = true)
 * - Cryto1 cipher is disabled (NP_ACTIVATE_CRYPTO1 = false)
 * - Easy framing is enabled (NP_EASY_FRAMING = true)
 * - Auto-switching in ISO14443-4 mode is enabled (NP_AUTO_ISO14443_4 = true)
 * - Invalid frames are not accepted (NP_ACCEPT_INVALID_FRAMES = false)
 * - Multiple frames are not accepted (NP_ACCEPT_MULTIPLE_FRAMES = false)
 * - 14443-A mode is activated (NP_FORCE_ISO14443_A = true)
 * - speed is set to 106 kbps (NP_FORCE_SPEED_106 = true)
 * - Let the device try forever to find a target (NP_INFINITE_SELECT = true)
 * - RF field is shortly dropped (if it was enabled) then activated again
 */
int nfc_initiator_init(nfc_device *pnd)
{
  static const struct property_bool_setting settings[] = {
      {NP_ACTIVATE_FIELD, false},
      {NP_ACTIVATE_FIELD, true},
      {NP_INFINITE_SELECT, true},
      {NP_AUTO_ISO14443_4, true},
      {NP_FORCE_ISO14443_A, true},
      {NP_FORCE_SPEED_106, true},
      {NP_ACCEPT_INVALID_FRAMES, false},
      {NP_ACCEPT_MULTIPLE_FRAMES, false}};

  int res = apply_property_sequence(pnd, settings, sizeof(settings) / sizeof(settings[0]));
  if (res < 0)
    return res;

  return HAL(initiator_init, pnd);
}

/** @ingroup initiator
 * @brief Initialize NFC device as initiator with its secure element as target (reader)
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 *
 * The NFC device is configured to function as secure element reader.
 * After initialization it can be used to communicate with the secure element.
 * @note RF field is deactivated in order to save some power
 */
int nfc_initiator_init_secure_element(nfc_device *pnd)
{
  return HAL(initiator_init_secure_element, pnd);
}

/** @ingroup initiator
 * @brief Select a passive or emulated tag
 * @return Returns selected passive target count on success, otherwise returns libnfc's error code (negative value)
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param nm desired modulation
 * @param pbtInitData optional initiator data, NULL for using the default values.
 * @param szInitData length of initiator data \a pbtInitData.
 * @note pbtInitData is used with different kind of data depending on modulation type:
 * - for an ISO/IEC 14443 type A modulation, pbbInitData contains the UID you want to select;
 * - for an ISO/IEC 14443 type B modulation, pbbInitData contains Application Family Identifier (AFI) (see ISO/IEC 14443-3)
        and optionally a second byte = 0x01 if you want to use probabilistic approach instead of timeslot approach;
 * - for a FeliCa modulation, pbbInitData contains a 5-byte polling payload (see ISO/IEC 18092 11.2.2.5).
 * - for ISO14443B', ASK CTx and ST SRx, see corresponding standards
 * - if NULL, default values adequate for the chosen modulation will be used.
 *
 * @param[out] pnt \a nfc_target struct pointer which will filled if available
 *
 * The NFC device will try to find one available passive tag or emulated tag.
 *
 * The chip needs to know with what kind of tag it is dealing with, therefore
 * the initial modulation and speed (106, 212 or 424 kbps) should be supplied.
 */
int nfc_initiator_select_passive_target(nfc_device *pnd,
                                        const nfc_modulation nm,
                                        const uint8_t *pbtInitData, const size_t szInitData,
                                        nfc_target *pnt)
{
  uint8_t *abtInit = NULL;
  size_t maxAbt = MAX((size_t)12, szInitData);
  size_t szInit = 0;
  int res;
  if ((res = nfc_device_validate_modulation(pnd, N_INITIATOR, &nm)) != NFC_SUCCESS)
  {
    return res;
  }
  if (szInitData == 0)
  {
    // Provide default values, if any
    prepare_initiator_data(nm, &abtInit, &szInit);
    return HAL(initiator_select_passive_target, pnd, nm, abtInit, szInit, pnt);
  }

  abtInit = malloc(sizeof(uint8_t) * maxAbt);
  if (nm.nmt == NMT_ISO14443A)
  {
    iso14443_cascade_uid(pbtInitData, szInitData, abtInit, &szInit);
  }
  else
  {
    if (nfc_safe_memcpy(abtInit, maxAbt, pbtInitData, szInitData) < 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to copy init data");
      free(abtInit);
      return NFC_EINVARG;
    }
    szInit = szInitData;
  }
  res = HAL(initiator_select_passive_target, pnd, nm, abtInit, szInit, pnt);
  free(abtInit);
  return res;
}

/** @ingroup initiator
 * @brief List passive or emulated tags
 * @return Returns the number of targets found on success, otherwise returns libnfc's error code (negative value)
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param nm desired modulation
 * @param[out] ant array of \a nfc_target that will be filled with targets info
 * @param szTargets size of \a ant (will be the max targets listed)
 *
 * The NFC device will try to find the available passive tags. Some NFC devices
 * are capable to emulate passive tags. The standards (ISO18092 and ECMA-340)
 * describe the modulation that can be used for reader to passive
 * communications. The chip needs to know with what kind of tag it is dealing
 * with, therefore the initial modulation and speed (106, 212 or 424 kbps)
 * should be supplied.
 */
int nfc_initiator_list_passive_targets(nfc_device *pnd,
                                       const nfc_modulation nm,
                                       nfc_target ant[], const size_t szTargets)
{
  if (szTargets == 0)
    return 0;

  uint8_t *pbtInitData = NULL;
  size_t szInitDataLen = 0;
  size_t target_count = 0;
  int res = 0;

  pnd->last_error = 0;

  bool restore_infinite = pnd->bInfiniteSelect;
  if ((res = nfc_device_set_property_bool(pnd, NP_INFINITE_SELECT, false)) < 0)
    return res;

  prepare_initiator_data(nm, &pbtInitData, &szInitDataLen);

  nfc_target nt;
  while (nfc_initiator_select_passive_target(pnd, nm, pbtInitData, szInitDataLen, &nt) > 0)
  {
    if (target_already_seen(ant, target_count, &nt))
      break;

    if (nfc_safe_memcpy(&ant[target_count], sizeof(nfc_target), &nt, sizeof(nfc_target)) < 0)
    {
      log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, "Failed to copy target data");
      res = NFC_EIO;
      break;
    }

    target_count++;
    if (target_count >= szTargets || modulation_requires_single_attempt(nm))
      break;

    nfc_initiator_deselect_target(pnd);
  }

  if (restore_infinite)
  {
    int restore_res = nfc_device_set_property_bool(pnd, NP_INFINITE_SELECT, true);
    if (restore_res < 0 && res >= 0)
      res = restore_res;
  }

  return (res < 0) ? res : (int)target_count;
}

/** @ingroup initiator
 * @brief Polling for NFC targets
 * @return Returns polled targets count, otherwise returns libnfc's error code (negative value).
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pnmModulations desired modulations
 * @param szModulations size of \a pnmModulations
 * @param uiPollNr specifies the number of polling (0x01 - 0xFE: 1 up to 254 polling, 0xFF: Endless polling)
 * @note one polling is a polling for each desired target type
 * @param uiPeriod indicates the polling period in units of 150 ms (0x01 - 0x0F: 150ms - 2.25s)
 * @note e.g. if uiPeriod=10, it will poll each desired target type during 1.5s
 * @param[out] pnt pointer on \a nfc_target (over)writable struct
 */
int nfc_initiator_poll_target(nfc_device *pnd,
                              const nfc_modulation *pnmModulations, const size_t szModulations,
                              const uint8_t uiPollNr, const uint8_t uiPeriod,
                              nfc_target *pnt)
{
  return HAL(initiator_poll_target, pnd, pnmModulations, szModulations, uiPollNr, uiPeriod, pnt);
}

/** @ingroup initiator
 * @brief Select a target and request active or passive mode for D.E.P. (Data Exchange Protocol)
 * @return Returns selected D.E.P targets count on success, otherwise returns libnfc's error code (negative value).
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param ndm desired D.E.P. mode (\a NDM_ACTIVE or \a NDM_PASSIVE for active, respectively passive mode)
 * @param nbr desired baud rate
 * @param pndiInitiator pointer \a nfc_dep_info struct that contains \e NFCID3 and \e General \e Bytes to set to the initiator device (optionnal, can be \e NULL)
 * @param[out] pnt is a \a nfc_target struct pointer where target information will be put.
 * @param timeout in milliseconds
 *
 * The NFC device will try to find an available D.E.P. target. The standards
 * (ISO18092 and ECMA-340) describe the modulation that can be used for reader
 * to passive communications.
 *
 * @note \a nfc_dep_info will be returned when the target was acquired successfully.
 *
 * If timeout equals to 0, the function blocks indefinitely (until an error is raised or function is completed)
 * If timeout equals to -1, the default timeout will be used
 */
int nfc_initiator_select_dep_target(nfc_device *pnd,
                                    const nfc_dep_mode ndm, const nfc_baud_rate nbr,
                                    const nfc_dep_info *pndiInitiator, nfc_target *pnt, const int timeout)
{
  return HAL(initiator_select_dep_target, pnd, ndm, nbr, pndiInitiator, pnt, timeout);
}

/** @ingroup initiator
 * @brief Poll a target and request active or passive mode for D.E.P. (Data Exchange Protocol)
 * @return Returns selected D.E.P targets count on success, otherwise returns libnfc's error code (negative value).
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param ndm desired D.E.P. mode (\a NDM_ACTIVE or \a NDM_PASSIVE for active, respectively passive mode)
 * @param nbr desired baud rate
 * @param pndiInitiator pointer \a nfc_dep_info struct that contains \e NFCID3 and \e General \e Bytes to set to the initiator device (optionnal, can be \e NULL)
 * @param[out] pnt is a \a nfc_target struct pointer where target information will be put.
 * @param timeout in milliseconds
 *
 * The NFC device will try to find an available D.E.P. target. The standards
 * (ISO18092 and ECMA-340) describe the modulation that can be used for reader
 * to passive communications.
 *
 * @note \a nfc_dep_info will be returned when the target was acquired successfully.
 */
int nfc_initiator_poll_dep_target(struct nfc_device *pnd,
                                  const nfc_dep_mode ndm, const nfc_baud_rate nbr,
                                  const nfc_dep_info *pndiInitiator,
                                  nfc_target *pnt,
                                  const int timeout)
{
  const int period = 300;
  int remaining_time = timeout;
  int res;
  int result = 0;
  bool bInfiniteSelect = pnd->bInfiniteSelect;
  if ((res = nfc_device_set_property_bool(pnd, NP_INFINITE_SELECT, true)) < 0)
    return res;
  while (remaining_time > 0)
  {
    if ((res = nfc_initiator_select_dep_target(pnd, ndm, nbr, pndiInitiator, pnt, period)) < 0)
    {
      if (res != NFC_ETIMEOUT)
      {
        result = res;
        goto end;
      }
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

/** @ingroup initiator
 * @brief Deselect a selected passive or emulated tag
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value).
 * @param pnd \a nfc_device struct pointer that represents currently used device
 *
 * After selecting and communicating with a passive tag, this function could be
 * used to deactivate and release the tag. This is very useful when there are
 * multiple tags available in the field. It is possible to use the \fn
 * nfc_initiator_select_passive_target() function to select the first available
 * tag, test it for the available features and support, deselect it and skip to
 * the next tag until the correct tag is found.
 */
int nfc_initiator_deselect_target(nfc_device *pnd)
{
  return HAL(initiator_deselect_target, pnd);
}

/** @ingroup initiator
 * @brief Send data to target then retrieve data from target
 * @return Returns received bytes count on success, otherwise returns libnfc's error code
 *
 * @param pnd \a nfc_device struct pointer that represents currently used device
 * @param pbtTx contains a byte array of the frame that needs to be transmitted.
 * @param szTx contains the length in bytes.
 * @param[out] pbtRx response from the target
 * @param szRx size of \a pbtRx (Will return NFC_EOVFLOW if RX exceeds this size)
 * @param timeout in milliseconds
 *
 * The NFC device (configured as initiator) will transmit the supplied bytes (\a pbtTx) to the target.
 * It waits for the response and stores the received bytes in the \a pbtRx byte array.
 *
 * If \a NP_EASY_FRAMING option is disabled the frames will sent and received in raw mode: \e PN53x will not handle input neither output data.
 *
 * The parity bits are handled by the \e PN53x chip. The CRC can be generated automatically or handled manually.
 * Using this function, frames can be communicated very fast via the NFC initiator to the tag.
 *
 * Tests show that on average this way of communicating is much faster than using the regular driver/middle-ware (often supplied by manufacturers).
 *
 * @warning The configuration option \a NP_HANDLE_PARITY must be set to \c true (the default value).
 *
 * @note When used with MIFARE Classic, NFC_EMFCAUTHFAIL error is returned if authentication command failed. You need to re-select the tag to operate with.
 *
 * If timeout equals to 0, the function blocks indefinitely (until an error is raised or function is completed)
 * If timeout equals to -1, the default timeout will be used
 */
int nfc_initiator_transceive_bytes(nfc_device *pnd, const uint8_t *pbtTx, const size_t szTx, uint8_t *pbtRx,
                                   const size_t szRx, int timeout)
{
  return HAL(initiator_transceive_bytes, pnd, pbtTx, szTx, pbtRx, szRx, timeout);
}

/** @ingroup initiator
 * @brief Transceive raw bit-frames to a target
 * @return Returns received bits count on success, otherwise returns libnfc's error code
 *
 * @param pnd \a nfc_device struct pointer that represents currently used device
 * @param pbtTx contains a byte array of the frame that needs to be transmitted.
 * @param szTxBits contains the length in bits.
 *
 * @note For example the REQA (0x26) command (first anti-collision command of
 * ISO14443-A) must be precise 7 bits long. This is not possible by using
 * nfc_initiator_transceive_bytes(). With that function you can only
 * communicate frames that consist of full bytes. When you send a full byte (8
 * bits + 1 parity) with the value of REQA (0x26), a tag will simply not
 * respond. More information about this can be found in the anti-collision
 * example (\e nfc-anticol).
 *
 * @param pbtTxPar parameter contains a byte array of the corresponding parity bits needed to send per byte.
 *
 * @note For example if you send the SELECT_ALL (0x93, 0x20) = [ 10010011,
 * 00100000 ] command, you have to supply the following parity bytes (0x01,
 * 0x00) to define the correct odd parity bits. This is only an example to
 * explain how it works, if you just are sending two bytes with ISO14443-A
 * compliant parity bits you better can use the
 * nfc_initiator_transceive_bytes() function.
 *
 * @param[out] pbtRx response from the target
 * @param szRx size of \a pbtRx (Will return NFC_EOVFLOW if RX exceeds this size)
 * @param[out] pbtRxPar parameter contains a byte array of the corresponding parity bits
 *
 * The NFC device (configured as \e initiator) will transmit low-level messages
 * where only the modulation is handled by the \e PN53x chip. Construction of
 * the frame (data, CRC and parity) is completely done by libnfc.  This can be
 * very useful for testing purposes. Some protocols (e.g. MIFARE Classic)
 * require to violate the ISO14443-A standard by sending incorrect parity and
 * CRC bytes. Using this feature you are able to simulate these frames.
 */
int nfc_initiator_transceive_bits(nfc_device *pnd,
                                  const uint8_t *pbtTx, const size_t szTxBits, const uint8_t *pbtTxPar,
                                  uint8_t *pbtRx, const size_t szRx,
                                  uint8_t *pbtRxPar)
{
  (void)szRx;
  return HAL(initiator_transceive_bits, pnd, pbtTx, szTxBits, pbtTxPar, pbtRx, pbtRxPar);
}

/** @ingroup initiator
 * @brief Send data to target then retrieve data from target
 * @return Returns received bytes count on success, otherwise returns libnfc's error code.
 *
 * @param pnd \a nfc_device struct pointer that represents currently used device
 * @param pbtTx contains a byte array of the frame that needs to be transmitted.
 * @param szTx contains the length in bytes.
 * @param[out] pbtRx response from the target
 * @param szRx size of \a pbtRx (Will return NFC_EOVFLOW if RX exceeds this size)
 *
 * This function is similar to nfc_initiator_transceive_bytes() with the following differences:
 * - A precise cycles counter will indicate the number of cycles between emission & reception of frames.
 * - It only supports mode with \a NP_EASY_FRAMING option disabled.
 * - Overall communication with the host is heavier and slower.
 *
 * Timer control:
 * By default timer configuration tries to maximize the precision, which also limits the maximum
 * cycles count before saturation/timeout.
 * E.g. with PN53x it can count up to 65535 cycles, so about 4.8ms, with a precision of about 73ns.
 * - If you're ok with the defaults, set *cycles = 0 before calling this function.
 * - If you need to count more cycles, set *cycles to the maximum you expect but don't forget
 *   you'll loose in precision and it'll take more time before timeout, so don't abuse!
 *
 * @warning The configuration option \a NP_EASY_FRAMING must be set to \c false.
 * @warning The configuration option \a NP_HANDLE_PARITY must be set to \c true (the default value).
 */
int nfc_initiator_transceive_bytes_timed(nfc_device *pnd,
                                         const uint8_t *pbtTx, const size_t szTx,
                                         uint8_t *pbtRx, const size_t szRx,
                                         uint32_t *cycles)
{
  return HAL(initiator_transceive_bytes_timed, pnd, pbtTx, szTx, pbtRx, szRx, cycles);
}

/** @ingroup initiator
 * @brief Check target presence
 * @return Returns 0 on success, otherwise returns libnfc's error code.
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pnt a \a nfc_target struct pointer where desired target information was stored (optionnal, can be \e NULL).
 * This function tests if \a nfc_target (or last selected tag if \e NULL) is currently present on NFC device.
 * @warning The target have to be selected before check its presence
 * @warning To run the test, one or more commands will be sent to target
 */
int nfc_initiator_target_is_present(nfc_device *pnd, const nfc_target *pnt)
{
  return HAL(initiator_target_is_present, pnd, pnt);
}

/** @ingroup initiator
 * @brief Transceive raw bit-frames to a target
 * @return Returns received bits count on success, otherwise returns libnfc's error code
 *
 * This function is similar to nfc_initiator_transceive_bits() with the following differences:
 * - A precise cycles counter will indicate the number of cycles between emission & reception of frames.
 * - It only supports mode with \a NP_EASY_FRAMING option disabled and CRC must be handled manually.
 * - Overall communication with the host is heavier and slower.
 *
 * Timer control:
 * By default timer configuration tries to maximize the precision, which also limits the maximum
 * cycles count before saturation/timeout.
 * E.g. with PN53x it can count up to 65535 cycles, so about 4.8ms, with a precision of about 73ns.
 * - If you're ok with the defaults, set *cycles = 0 before calling this function.
 * - If you need to count more cycles, set *cycles to the maximum you expect but don't forget
 *   you'll loose in precision and it'll take more time before timeout, so don't abuse!
 *
 * @warning The configuration option \a NP_EASY_FRAMING must be set to \c false.
 * @warning The configuration option \a NP_HANDLE_CRC must be set to \c false.
 * @warning The configuration option \a NP_HANDLE_PARITY must be set to \c true (the default value).
 */
int nfc_initiator_transceive_bits_timed(nfc_device *pnd,
                                        const uint8_t *pbtTx, const size_t szTxBits, const uint8_t *pbtTxPar,
                                        uint8_t *pbtRx, const size_t szRx,
                                        uint8_t *pbtRxPar,
                                        uint32_t *cycles)
{
  (void)szRx;
  return HAL(initiator_transceive_bits_timed, pnd, pbtTx, szTxBits, pbtTxPar, pbtRx, pbtRxPar, cycles);
}

/** @ingroup target
 * @brief Initialize NFC device as an emulated tag
 * @return Returns received bytes count on success, otherwise returns libnfc's error code
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pnt pointer to \a nfc_target struct that represents the wanted emulated target
 *
 * @note \a pnt can be updated by this function: if you set NBR_UNDEFINED
 * and/or NDM_UNDEFINED (ie. for DEP mode), these fields will be updated.
 *
 * @param[out] pbtRx Rx buffer pointer
 * @param[out] szRx received bytes count
 * @param timeout in milliseconds
 *
 * This function initializes NFC device in \e target mode in order to emulate a
 * tag using the specified \a nfc_target_mode_t.
 * - Crc is handled by the device (NP_HANDLE_CRC = true)
 * - Parity is handled the device (NP_HANDLE_PARITY = true)
 * - Cryto1 cipher is disabled (NP_ACTIVATE_CRYPTO1 = false)
 * - Auto-switching in ISO14443-4 mode is enabled (NP_AUTO_ISO14443_4 = true)
 * - Easy framing is disabled (NP_EASY_FRAMING = false)
 * - Invalid frames are not accepted (NP_ACCEPT_INVALID_FRAMES = false)
 * - Multiple frames are not accepted (NP_ACCEPT_MULTIPLE_FRAMES = false)
 * - RF field is dropped
 *
 * @warning Be aware that this function will wait (hang) until a command is
 * received that is not part of the anti-collision. The RATS command for
 * example would wake up the emulator. After this is received, the send and
 * receive functions can be used.
 *
 * If timeout equals to 0, the function blocks indefinitely (until an error is raised or function is completed)
 * If timeout equals to -1, the default timeout will be used
 */
int nfc_target_init(nfc_device *pnd, nfc_target *pnt, uint8_t *pbtRx, const size_t szRx, int timeout)
{
  static const struct property_bool_setting settings[] = {
      {NP_ACCEPT_INVALID_FRAMES, false},
      {NP_ACCEPT_MULTIPLE_FRAMES, false},
      {NP_HANDLE_CRC, true},
      {NP_HANDLE_PARITY, true},
      {NP_AUTO_ISO14443_4, true},
      {NP_EASY_FRAMING, true},
      {NP_ACTIVATE_CRYPTO1, false},
      {NP_ACTIVATE_FIELD, false}};

  int res = apply_property_sequence(pnd, settings, sizeof(settings) / sizeof(settings[0]));
  if (res < 0)
    return res;

  return HAL(target_init, pnd, pnt, pbtRx, szRx, timeout);
}

/** @ingroup dev
 * @brief Turn NFC device in idle mode
 * @return Returns 0 on success, otherwise returns libnfc's error code.
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 *
 * This function switch the device in idle mode.
 * In initiator mode, the RF field is turned off and the device is set to low power mode (if available);
 * In target mode, the emulation is stoped (no target available from external initiator) and the device is set to low power mode (if avaible).
 */
int nfc_idle(nfc_device *pnd)
{
  return HAL(idle, pnd);
}

/** @ingroup dev
 * @brief Abort current running command
 * @return Returns 0 on success, otherwise returns libnfc's error code.
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 *
 * Some commands (ie. nfc_target_init()) are blocking functions and will return only in particular conditions (ie. external initiator request).
 * This function attempt to abort the current running command.
 *
 * @note The blocking function (ie. nfc_target_init()) will failed with DEABORT error.
 */
int nfc_abort_command(nfc_device *pnd)
{
  return HAL(abort_command, pnd);
}

/** @ingroup target
 * @brief Send bytes and APDU frames
 * @return Returns sent bytes count on success, otherwise returns libnfc's error code
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pbtTx pointer to Tx buffer
 * @param szTx size of Tx buffer
 * @param timeout in milliseconds
 *
 * This function make the NFC device (configured as \e target) send byte frames
 * (e.g. APDU responses) to the \e initiator.
 *
 * If timeout equals to 0, the function blocks indefinitely (until an error is raised or function is completed)
 * If timeout equals to -1, the default timeout will be used
 */
int nfc_target_send_bytes(nfc_device *pnd, const uint8_t *pbtTx, const size_t szTx, int timeout)
{
  return HAL(target_send_bytes, pnd, pbtTx, szTx, timeout);
}

/** @ingroup target
 * @brief Receive bytes and APDU frames
 * @return Returns received bytes count on success, otherwise returns libnfc's error code
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pbtRx pointer to Rx buffer
 * @param szRx size of Rx buffer
 * @param timeout in milliseconds
 *
 * This function retrieves bytes frames (e.g. ADPU) sent by the \e initiator to the NFC device (configured as \e target).
 *
 * If timeout equals to 0, the function blocks indefinitely (until an error is raised or function is completed)
 * If timeout equals to -1, the default timeout will be used
 */
int nfc_target_receive_bytes(nfc_device *pnd, uint8_t *pbtRx, const size_t szRx, int timeout)
{
  return HAL(target_receive_bytes, pnd, pbtRx, szRx, timeout);
}

/** @ingroup target
 * @brief Send raw bit-frames
 * @return Returns sent bits count on success, otherwise returns libnfc's error code.
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pbtTx pointer to Tx buffer
 * @param szTxBits size of Tx buffer
 * @param pbtTxPar parameter contains a byte array of the corresponding parity bits needed to send per byte.
 * This function can be used to transmit (raw) bit-frames to the \e initiator
 * using the specified NFC device (configured as \e target).
 */
int nfc_target_send_bits(nfc_device *pnd, const uint8_t *pbtTx, const size_t szTxBits, const uint8_t *pbtTxPar)
{
  return HAL(target_send_bits, pnd, pbtTx, szTxBits, pbtTxPar);
}

/** @ingroup target
 * @brief Receive bit-frames
 * @return Returns received bits count on success, otherwise returns libnfc's error code
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pbtRx pointer to Rx buffer
 * @param szRx size of Rx buffer
 * @param[out] pbtRxPar parameter contains a byte array of the corresponding parity bits
 *
 * This function makes it possible to receive (raw) bit-frames.  It returns all
 * the messages that are stored in the FIFO buffer of the \e PN53x chip.  It
 * does not require to send any frame and thereby could be used to snoop frames
 * that are transmitted by a nearby \e initiator.  @note Check out the
 * NP_ACCEPT_MULTIPLE_FRAMES configuration option to avoid losing transmitted
 * frames.
 */
int nfc_target_receive_bits(nfc_device *pnd, uint8_t *pbtRx, const size_t szRx, uint8_t *pbtRxPar)
{
  return HAL(target_receive_bits, pnd, pbtRx, szRx, pbtRxPar);
}

static struct sErrorMessage
{
  int iErrorCode;
  const char *pcErrorMsg;
} sErrorMessages[] = {
    /* Chip-level errors (internal errors, RF errors, etc.) */
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

/** @ingroup error
 * @brief Return the last error string
 * @return Returns a string
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 */
const char *
nfc_strerror(const nfc_device *pnd)
{
  const char *pcRes = "Unknown error";
  size_t i;
  for (i = 0; i < (sizeof(sErrorMessages) / sizeof(struct sErrorMessage)); i++)
  {
    if (sErrorMessages[i].iErrorCode == pnd->last_error)
    {
      pcRes = sErrorMessages[i].pcErrorMsg;
      break;
    }
  }

  return pcRes;
}

/** @ingroup error
 * @brief Renders the last error in pcStrErrBuf for a maximum size of szBufLen chars
 * @return Returns 0 upon success
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pcStrErrBuf a string that contains the last error.
 * @param szBufLen size of buffer
 */
int nfc_strerror_r(const nfc_device *pnd, char *pcStrErrBuf, size_t szBufLen)
{
  return (snprintf(pcStrErrBuf, szBufLen, "%s", nfc_strerror(pnd)) < 0) ? -1 : 0;
}

/** @ingroup error
 * @brief Display the last error occured on a nfc_device
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param pcString a string
 */
void nfc_perror(const nfc_device *pnd, const char *pcString)
{
  fprintf(stderr, "%s: %s\n", pcString, nfc_strerror(pnd));
}

/** @ingroup error
 * @brief Returns last error occured on a nfc_device
 * @return Returns an integer that represents to libnfc's error code.
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 */
int nfc_device_get_last_error(const nfc_device *pnd)
{
  return pnd->last_error;
}

/* Special data accessors */

/** @ingroup data
 * @brief Returns the device name
 * @return Returns a string with the device name
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 */
const char *
nfc_device_get_name(nfc_device *pnd)
{
  return pnd->name;
}

/** @ingroup data
 * @brief Returns the device connection string
 * @return Returns a string with the device connstring
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 */
const char *
nfc_device_get_connstring(nfc_device *pnd)
{
  return pnd->connstring;
}

/** @ingroup data
 * @brief Get supported modulations.
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param mode \a nfc_mode.
 * @param supported_mt pointer of \a nfc_modulation_type array.
 *
 */
int nfc_device_get_supported_modulation(nfc_device *pnd, const nfc_mode mode, const nfc_modulation_type **const supported_mt)
{
  return HAL(get_supported_modulation, pnd, mode, supported_mt);
}

/** @ingroup data
 * @brief Get supported baud rates (initiator mode).
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param nmt \a nfc_modulation_type.
 * @param supported_br pointer of \a nfc_baud_rate array.
 *
 */
int nfc_device_get_supported_baud_rate(nfc_device *pnd, const nfc_modulation_type nmt, const nfc_baud_rate **const supported_br)
{
  return HAL(get_supported_baud_rate, pnd, N_INITIATOR, nmt, supported_br);
}

/** @ingroup data
 * @brief Get supported baud rates for target mode.
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param nmt \a nfc_modulation_type.
 * @param supported_br pointer of \a nfc_baud_rate array.
 *
 */
int nfc_device_get_supported_baud_rate_target_mode(nfc_device *pnd, const nfc_modulation_type nmt, const nfc_baud_rate **const supported_br)
{
  return HAL(get_supported_baud_rate, pnd, N_TARGET, nmt, supported_br);
}

/** @ingroup data
 * @brief Validate combination of modulation and baud rate on the currently used device.
 * @return Returns 0 on success, otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param mode \a nfc_mode.
 * @param nm \a nfc_modulation.
 *
 */
static int
nfc_device_validate_modulation(nfc_device *pnd, const nfc_mode mode, const nfc_modulation *nm)
{
  const nfc_modulation_type *supported_types = NULL;
  int res = nfc_device_get_supported_modulation(pnd, mode, &supported_types);
  if (res < 0)
    return res;

  if (!modulation_supported(supported_types, nm->nmt))
  {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "nfc_device_validate_modulation returning NFC_EINVARG");
    return NFC_EINVARG;
  }

  const nfc_baud_rate *supported_rates = get_baud_rates_for_mode(pnd, mode, nm->nmt, &res);
  if (res < 0)
    return res;

  if (baud_rate_supported(supported_rates, nm->nbr))
    return NFC_SUCCESS;

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, "nfc_device_validate_modulation returning NFC_EINVARG");
  return NFC_EINVARG;
}

/* Misc. functions */

/** @ingroup misc
 * @brief Returns the library version
 * @return Returns a string with the library version
 *
 * @param pnd \a nfc_device struct pointer that represent currently used device
 */
const char *
nfc_version(void)
{
#ifdef GIT_REVISION
  return GIT_REVISION;
#else
  return PACKAGE_VERSION;
#endif // GIT_REVISION
}

/** @ingroup misc
 * @brief Free buffer allocated by libnfc
 *
 * @param pointer on buffer that needs to be freed
 */
void nfc_free(void *p)
{
  free(p);
}

/** @ingroup misc
 * @brief Print information about NFC device
 * @return Upon successful return, this function returns the number of characters printed (excluding the null byte used to end output to strings), otherwise returns libnfc's error code (negative value)
 * @param pnd \a nfc_device struct pointer that represent currently used device
 * @param buf pointer where string will be allocated, then information printed
 *
 * @warning *buf must be freed using nfc_free()
 */
int nfc_device_get_information_about(nfc_device *pnd, char **buf)
{
  return HAL(device_get_information_about, pnd, buf);
}

/** @ingroup string-converter
 * @brief Convert \a nfc_baud_rate value to string
 * @return Returns nfc baud rate
 * @param nbr \a nfc_baud_rate to convert
 */
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

/** @ingroup string-converter
 * @brief Convert \a nfc_modulation_type value to string
 * @return Returns nfc modulation type
 * @param nmt \a nfc_modulation_type to convert
 */
const char *
str_nfc_modulation_type(const nfc_modulation_type nmt)
{
  return lookup_modulation_type_name(nmt);
}

/** @ingroup string-converter
 * @brief Convert \a nfc_target content to string
 * @return Upon successful return, this function returns the number of characters printed (excluding the null byte used to end output to strings), otherwise returns libnfc's error code (negative value)
 * @param pnt \a nfc_target struct pointer to print
 * @param buf pointer where string will be allocated, then nfc target information printed
 * @param verbose false for essential, true for detailed, human-readable, information
 *
 * @warning *buf must be freed using nfc_free()
 */
int str_nfc_target(char **buf, const nfc_target *pnt, bool verbose)
{
  *buf = malloc(4096);
  if (!*buf)
    return NFC_ESOFT;
  (*buf)[0] = '\0';
  snprint_nfc_target(*buf, 4096, pnt, verbose);
  return (int)nfc_safe_strlen(*buf, 4096);
}
