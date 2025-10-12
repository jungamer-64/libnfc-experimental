/**
 * @file nfc-common.c
 * @brief Implementation of common utility functions
 *
 * This file implements common patterns extracted from multiple drivers
 * to reduce code duplication from 31% to <15%.
 *
 * C23 Optimizations:
 * - Improved code organization and readability
 * - Better error handling with consistent logging
 * - Reduced code duplication through helper functions
 * - Enhanced type safety
 */

#include "nfc-common.h"
#include "chips/pn53x.h"
#include <stdio.h>
#include <string.h>

#define LOG_GROUP NFC_LOG_GROUP_GENERAL
#define LOG_CATEGORY "libnfc.common"

/* ============================================================================
 * DEVICE INITIALIZATION ERROR HANDLING
 * ========================================================================== */

/**
 * @brief Comprehensive device initialization error handler
 */
int nfc_device_init_failed(nfc_device *pnd,
                           void *port,
                           port_close_fn close_fn,
                           void **ports,
                           bool chip_data_allocated)
{
  /* Close communication port if provided */
  if (port != NULL && close_fn != NULL) {
    close_fn(port);
  }

  /* Free chip-specific data if it was allocated */
  if (pnd != NULL && chip_data_allocated) {
    pn53x_data_free(pnd);
  }

  /* Free device structure */
  if (pnd != NULL) {
    nfc_device_free(pnd);
  }

  /* Clean up port array */
  nfc_free_array(ports);

  return 0; /* Return value for scan functions */
}

/**
 * @brief Common resource cleanup for device open failures
 */
void nfc_device_open_failed(nfc_device *pnd,
                            void *driver_data,
                            bool chip_data_allocated)
{
  if (pnd == NULL) {
    /* Device structure not allocated, free driver_data directly */
    if (driver_data != NULL) {
      free(driver_data);
    }
    return;
  }

  /* Free chip-specific data if allocated */
  if (chip_data_allocated) {
    pn53x_data_free(pnd);
  }

  /* nfc_device_free will handle driver_data */
  nfc_device_free(pnd);
}

/* ============================================================================
 * CONNECTION STRING PARSING
 * ========================================================================== */

/**
 * @brief Extract connection string components
 */
int nfc_parse_connstring(const char *connstring,
                         const char *prefix,
                         const char *param_name,
                         char *param_value,
                         size_t param_value_size)
{
  /* Validate input parameters */
  if (connstring == NULL || prefix == NULL ||
      param_name == NULL || param_value == NULL ||
      param_value_size == 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Invalid parameters for connstring parsing");
    return -1;
  }

  /* Check if connstring starts with expected prefix */
  const size_t prefix_len = strlen(prefix);
  if (strncmp(connstring, prefix, prefix_len) != 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "Connstring '%s' does not match prefix '%s'",
            connstring, prefix);
    return -1;
  }

  /* Look for param_name in connstring */
  /* Format: "prefix:param_name=value" or "prefix:param_name=value:other=value" */
  const char *param_start = connstring + prefix_len;

  /* Skip colon after prefix */
  if (*param_start == ':') {
    param_start++;
  }

  /* Build parameter search pattern "param_name=" */
  char search_pattern[128];
  const int ret = snprintf(search_pattern, sizeof(search_pattern),
                           "%s=", param_name);
  if (ret < 0 || (size_t)ret >= sizeof(search_pattern)) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Parameter name too long: %s", param_name);
    return -1;
  }

  /* Find parameter in connstring */
  const char *param_pos = strstr(param_start, search_pattern);
  if (param_pos == NULL) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "Parameter '%s' not found in connstring '%s'",
            param_name, connstring);
    return -1;
  }

  /* Move to value part (after '=') */
  const char *value_start = param_pos + strlen(search_pattern);

  /* Find end of value (next ':' or end of string) */
  const char *value_end = strchr(value_start, ':');
  size_t value_len;

  if (value_end != NULL) {
    value_len = (size_t)(value_end - value_start);
  } else {
    value_len = strlen(value_start);
  }

  /* Check buffer size */
  if (value_len >= param_value_size) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Parameter value too long (%zu >= %zu)",
            value_len, param_value_size);
    return -1;
  }

  /* Copy value using safe memcpy */
  if (nfc_safe_memcpy(param_value, param_value_size,
                      value_start, value_len) < 0) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Failed to copy parameter value");
    return -1;
  }

  /* Null-terminate */
  param_value[value_len] = '\0';

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "Extracted parameter '%s'='%s' from connstring",
          param_name, param_value);

  return 0;
}

/**
 * @brief Build standardized connection string
 */
int nfc_build_connstring(char *dest,
                         size_t dest_size,
                         const char *driver_name,
                         const char *param_name,
                         const char *param_value)
{
  /* Validate input parameters */
  if (dest == NULL || dest_size == 0 ||
      driver_name == NULL || param_name == NULL ||
      param_value == NULL) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Invalid parameters for connstring building");
    return -1;
  }

  /* Format: "driver_name:param_name=param_value" */
  const int ret = snprintf(dest, dest_size, "%s:%s=%s",
                           driver_name, param_name, param_value);

  if (ret < 0 || (size_t)ret >= dest_size) {
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
            "Connection string buffer overflow (need %d bytes, have %zu)",
            ret, dest_size);
    return -1;
  }

  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
          "Built connection string: '%s'", dest);

  return 0;
}
