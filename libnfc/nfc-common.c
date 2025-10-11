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
 * @file nfc-common.c
 * @brief Implementation of common utility functions
 *
 * This file implements common patterns extracted from multiple drivers
 * to reduce code duplication from 31% to <15%.
 */

#include "nfc-common.h"
#include "chips/pn53x.h" /* For pn53x_data_free */
#include <stdio.h>
#include <string.h>

#define LOG_GROUP NFC_LOG_GROUP_GENERAL
#define LOG_CATEGORY "libnfc.common"

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
    if (port && close_fn)
    {
        close_fn(port);
    }

    /* Free chip-specific data if it was allocated */
    if (pnd && chip_data_allocated)
    {
        pn53x_data_free(pnd);
    }

    /* Free device structure */
    if (pnd)
    {
        nfc_device_free(pnd);
    }

    /* Clean up port array */
    nfc_free_array(ports);

    return 0; /* Return value for scan functions */
}

/**
 * @brief Extract connection string components
 */
int nfc_parse_connstring(const char *connstring,
                         const char *prefix,
                         const char *param_name,
                         char *param_value,
                         size_t param_value_size)
{
    if (!connstring || !prefix || !param_name || !param_value || param_value_size == 0)
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Invalid parameters for connstring parsing");
        return -1;
    }

    /* Check if connstring starts with expected prefix */
    size_t prefix_len = strlen(prefix);
    if (strncmp(connstring, prefix, prefix_len) != 0)
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
                "Connstring '%s' does not match prefix '%s'", connstring, prefix);
        return -1;
    }

    /* Look for param_name in connstring */
    /* Format: "prefix:param_name=value" or "prefix:param_name=value:other=value" */
    const char *param_start = connstring + prefix_len;

    /* Skip colon after prefix */
    if (*param_start == ':')
    {
        param_start++;
    }

    /* Build parameter search pattern "param_name=" */
    char search_pattern[128];
    int ret = snprintf(search_pattern, sizeof(search_pattern), "%s=", param_name);
    if (ret < 0 || (size_t)ret >= sizeof(search_pattern))
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Parameter name too long: %s", param_name);
        return -1;
    }

    /* Find parameter in connstring */
    const char *param_pos = strstr(param_start, search_pattern);
    if (!param_pos)
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
                "Parameter '%s' not found in connstring '%s'", param_name, connstring);
        return -1;
    }

    /* Move to value part (after '=') */
    const char *value_start = param_pos + strlen(search_pattern);

    /* Find end of value (next ':' or end of string) */
    const char *value_end = strchr(value_start, ':');
    size_t value_len;

    if (value_end)
    {
        value_len = value_end - value_start;
    }
    else
    {
        value_len = strlen(value_start);
    }

    /* Check buffer size */
    if (value_len >= param_value_size)
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Parameter value too long (%zu >= %zu)", value_len, param_value_size);
        return -1;
    }

    /* Copy value using safe memcpy */
    if (nfc_safe_memcpy(param_value, param_value_size,
                        value_start, value_len) < 0)
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Failed to copy parameter value");
        return -1;
    }

    /* Null-terminate */
    param_value[value_len] = '\0';

    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "Extracted parameter '%s'='%s' from connstring", param_name, param_value);

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
    if (!dest || dest_size == 0 || !driver_name || !param_name || !param_value)
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Invalid parameters for connstring building");
        return -1;
    }

    /* Format: "driver_name:param_name=param_value" */
    int ret = snprintf(dest, dest_size, "%s:%s=%s",
                       driver_name, param_name, param_value);

    if (ret < 0 || (size_t)ret >= dest_size)
    {
        log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR,
                "Connection string buffer overflow (need %d bytes, have %zu)",
                ret, dest_size);
        return -1;
    }

    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG,
            "Built connection string: '%s'", dest);

    return 0;
}

/**
 * @brief Common resource cleanup for device open failures
 */
void nfc_device_open_failed(nfc_device *pnd,
                            void *driver_data,
                            bool chip_data_allocated)
{
    if (!pnd)
    {
        /* Device structure not allocated, free driver_data directly */
        if (driver_data)
        {
            free(driver_data);
        }
        return;
    }

    /* Free chip-specific data if allocated */
    if (chip_data_allocated)
    {
        pn53x_data_free(pnd);
    }

    /* nfc_device_free will handle driver_data */
    nfc_device_free(pnd);
}
