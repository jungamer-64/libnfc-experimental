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
 * Copyright (C) 2025      jgm
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
 * @file nfc-common.h
 * @brief Common utility functions to reduce code duplication across drivers
 *
 * This file contains extracted common patterns from multiple drivers:
 * - Device initialization helpers
 * - Resource cleanup patterns
 * - Error handling macros
 * - Logging convenience functions
 *
 * Purpose: Reduce code duplication from 31% to <15%
 * Target: Extract 10+ common patterns used across drivers
 * Phase: 11 - Code Quality Enhancement
 */

#ifndef __NFC_COMMON_H__
#define __NFC_COMMON_H__

#include <stdlib.h>
#include <string.h>
#include <errno.h>

#ifndef WIN32
#include <unistd.h> /* For pipe(), close() */
#endif

#include "nfc/nfc.h"
#include "nfc-internal.h"
#include "nfc-secure.h" /* For nfc_safe_memcpy */
#include "log.h"

#ifdef __cplusplus
extern "C"
{
#endif

/**
 * @brief Common error codes used across drivers
 */
#define NFC_COMMON_SUCCESS 0
#define NFC_COMMON_ERROR -1
#define NFC_COMMON_NOMEM -ENOMEM
#define NFC_COMMON_INVALID -EINVAL

/**
 * @brief Error logging and return macro
 *
 * Combines error logging and return statement to reduce repetition.
 *
 * @param error_code The error code to return (e.g., NFC_ESOFT, NFC_EIO)
 * @param format printf-style format string for log message
 * @param ... Variable arguments for format string
 *
 * @example
 *   if (buffer_size < required_size) {
 *       NFC_LOG_ERROR_AND_RETURN(NFC_ESOFT, "Buffer too small: %zu < %zu",
 *                                buffer_size, required_size);
 *   }
 */
#define NFC_LOG_ERROR_AND_RETURN(error_code, format, ...)                                \
  do                                                                                   \
  {                                                                                    \
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, format, ##__VA_ARGS__); \
    return (error_code);                                                             \
  } while (0)

/**
 * @brief Warning logging macro with descriptive message
 */
#define NFC_LOG_WARN(format, ...) \
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN, format, ##__VA_ARGS__)

/**
 * @brief Info logging macro for common scenarios
 */
#define NFC_LOG_INFO(format, ...) \
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, format, ##__VA_ARGS__)

/**
 * @brief Debug logging macro (only in debug builds)
 */
#define NFC_LOG_DEBUG(format, ...) \
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, format, ##__VA_ARGS__)

/**
 * @brief Safe array cleanup helper
 *
 * Frees all elements in a NULL-terminated array, then frees the array itself.
 * Commonly used for port/device arrays.
 *
 * @param array The NULL-terminated array of pointers to free
 *
 * @example
 *   const char **ports = get_serial_ports();
 *   // ... use ports ...
 *   nfc_free_array((void**)ports);
 */
static inline void
nfc_free_array(void **array)
{
  if (!array)
    return;

  int i = 0;
  while (array[i]) {
    free(array[i]);
    i++;
  }
  free(array);
}

/**
 * @brief Device cleanup helper pattern
 *
 * Common cleanup sequence used by many drivers when device initialization fails.
 * Handles port array cleanup and returns appropriate value.
 *
 * @param ports NULL-terminated array of port strings to free
 * @param return_value Value to return after cleanup
 * @return The specified return_value
 *
 * @example
 *   if (pn53x_data_new(pnd, &driver_io) == NULL) {
 *       perror("malloc");
 *       uart_close(port);
 *       nfc_device_free(pnd);
 *       return nfc_cleanup_and_return((void**)acPorts, 0);
 *   }
 */
static inline int
nfc_cleanup_and_return(void **ports, int return_value)
{
  nfc_free_array(ports);
  return return_value;
}

/**
 * @brief Allocate and initialize driver data
 *
 * Common pattern: allocate driver_data structure and check for errors.
 *
 * @param pnd The device pointer whose driver_data will be allocated
 * @param data_size Size of the driver-specific data structure
 * @return 0 on success, -1 on failure
 *
 * @example
 *   if (nfc_alloc_driver_data(pnd, sizeof(struct pn53x_usb_data)) < 0) {
 *       // Cleanup already logged
 *       goto error;
 *   }
 */
static inline int
nfc_alloc_driver_data(nfc_device *pnd, size_t data_size)
{
  if (!pnd || data_size == 0) {
    log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
            "Invalid parameters for driver data allocation");
    return -1;
  }

  pnd->driver_data = malloc(data_size);
  if (!pnd->driver_data) {
    perror("malloc");
    log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
            "Failed to allocate driver data (%zu bytes)", data_size);
    return -1;
  }

  // Zero-initialize for safety
  memset(pnd->driver_data, 0, data_size);
  return 0;
}

/**
 * @brief Comprehensive device initialization error handler
 *
 * Centralizes the repetitive error handling pattern seen across all drivers:
 * 1. Close communication port
 * 2. Free chip data
 * 3. Free device structure
 * 4. Clean up port array
 *
 * @param pnd Device pointer to clean up (can be NULL)
 * @param port Communication port to close (driver-specific, can be NULL)
 * @param close_fn Function pointer to close the port (uart_close, usb_close, etc.)
 * @param ports Port array to free
 * @param chip_data_allocated Whether chip data was allocated (true = call pn53x_data_free)
 * @return Always returns 0 for use in scan functions
 *
 * @example
 *   if (pn53x_check_communication(pnd) < 0) {
 *       return nfc_device_init_failed(pnd, sp, uart_close, acPorts, true);
 *   }
 */
typedef void (*port_close_fn)(void *port);

int nfc_device_init_failed(nfc_device *pnd,
                           void *port,
                           port_close_fn close_fn,
                           void **ports,
                           bool chip_data_allocated);

/**
 * @brief Extract connection string components
 *
 * Many drivers parse connstrings in similar ways. This helper extracts common patterns.
 *
 * @param connstring The connection string to parse
 * @param prefix Expected prefix (e.g., "pn53x_usb")
 * @param param_name Parameter name to extract (e.g., "port", "vid", "pid")
 * @param param_value Buffer to store extracted parameter value
 * @param param_value_size Size of param_value buffer
 * @return 0 on success, -1 if parameter not found or invalid
 *
 * @example
 *   char port_name[256];
 *   if (nfc_parse_connstring(connstring, "pn532_uart", "port",
 *                            port_name, sizeof(port_name)) == 0) {
 *       // port_name now contains the port parameter
 *   }
 */
int nfc_parse_connstring(const char *connstring,
                         const char *prefix,
                         const char *param_name,
                         char *param_value,
                         size_t param_value_size);

/**
 * @brief Validate device pointer before operations
 *
 * Common check pattern at beginning of driver functions.
 *
 * @param pnd Device pointer to validate
 * @param function_name Name of calling function (for error message)
 * @return true if valid, false if NULL
 *
 * @example
 *   static int my_driver_send(nfc_device *pnd, const uint8_t *data, size_t len) {
 *       if (!nfc_device_validate(pnd, "my_driver_send")) {
 *           return NFC_EIO;
 *       }
 *       // ... proceed with operation ...
 *   }
 */
static inline bool
nfc_device_validate(const nfc_device *pnd, const char *function_name)
{
  if (pnd == NULL) {
    log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
            "%s: NULL device pointer", function_name ? function_name : "unknown");
    return false;
  }
  return true;
}

/**
 * @brief Abort mechanism initialization helper (POSIX systems)
 *
 * Common pattern for initializing pipe-based abort mechanism on Unix-like systems.
 *
 * @param abort_fds Array of 2 file descriptors for the pipe
 * @return 0 on success, -1 on failure
 *
 * @example
 *   #ifndef WIN32
 *   if (nfc_init_abort_mechanism(DRIVER_DATA(pnd)->abort_fds) < 0) {
 *       // Cleanup
 *       return NULL;
 *   }
 *   #else
 *   DRIVER_DATA(pnd)->abort_flag = false;
 *   #endif
 */
#ifndef WIN32
static inline int
nfc_init_abort_mechanism(int abort_fds[2])
{
  if (pipe(abort_fds) < 0) {
    log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
            "Failed to create abort pipe: %s", strerror(errno));
    return -1;
  }
  return 0;
}

/**
 * @brief Abort mechanism cleanup helper (POSIX systems)
 *
 * Closes both ends of abort mechanism pipe.
 *
 * @param abort_fds Array of 2 file descriptors for the pipe
 */
static inline void
nfc_close_abort_mechanism(int abort_fds[2])
{
  if (abort_fds[0] >= 0)
    close(abort_fds[0]);
  if (abort_fds[1] >= 0)
    close(abort_fds[1]);
}
#endif /* WIN32 */

/**
 * @brief Build standardized connection string
 *
 * Formats connection string following libnfc conventions.
 *
 * @param dest Destination buffer
 * @param dest_size Size of destination buffer
 * @param driver_name Driver name (e.g., "pn53x_usb")
 * @param param_name Parameter name (e.g., "port")
 * @param param_value Parameter value (e.g., "/dev/ttyUSB0")
 * @return 0 on success, -1 on overflow or invalid parameters
 *
 * @example
 *   nfc_connstring connstring;
 *   nfc_build_connstring(connstring, sizeof(connstring),
 *                        "pn532_uart", "port", "/dev/ttyUSB0");
 *   // Result: "pn532_uart:port=/dev/ttyUSB0"
 */
int nfc_build_connstring(char *dest,
                         size_t dest_size,
                         const char *driver_name,
                         const char *param_name,
                         const char *param_value);

/**
 * @brief Copy connection string safely with validation
 *
 * Wraps nfc_safe_memcpy specifically for connection strings with logging.
 *
 * @param dest Destination connstring buffer
 * @param src Source connstring
 * @return 0 on success, -1 on failure
 *
 * @example
 *   if (nfc_copy_connstring(connstrings[device_found], connstring) < 0) {
 *       continue;  // Try next device
 *   }
 *   device_found++;
 */
static inline int
nfc_copy_connstring(nfc_connstring dest, const nfc_connstring src)
{
  if (nfc_safe_memcpy(dest, sizeof(nfc_connstring),
                      src, sizeof(nfc_connstring)) < 0) {
    log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
            "Failed to copy connection string");
    return -1;
  }
  return 0;
}

/**
 * @brief Common resource cleanup for device open failures
 *
 * Pattern extracted from multiple driver open functions.
 * Handles cleanup when device initialization fails midway.
 *
 * @param pnd Device to free (can be NULL)
 * @param driver_data Driver-specific data (freed if pnd is NULL)
 * @param chip_data_allocated Whether chip data needs freeing
 */
void nfc_device_open_failed(nfc_device *pnd,
                            void *driver_data,
                            bool chip_data_allocated);

#ifdef __cplusplus
}
#endif

#endif /* __NFC_COMMON_H__ */
