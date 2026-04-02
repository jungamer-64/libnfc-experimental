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
 * Copyright (C) 2025-2026 jungamer-64
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

#include "log.h"
#include "rust_bridge.h"

#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#ifndef va_copy
#define va_copy(dst, src) ((dst) = (src))
#endif

static const char LOG_FORMATTING_FAILED[] = "<log formatting failed>";

static void
log_dispatch_message(uint8_t group, const char *category, uint8_t priority, const char *message)
{
  nfc_rs_log_message(group, category, priority, message);
}

const char *
log_priority_to_str(const int priority)
{
  switch (priority) {
    case NFC_LOG_PRIORITY_ERROR:
      return "error";
    case NFC_LOG_PRIORITY_INFO:
      return "info";
    case NFC_LOG_PRIORITY_DEBUG:
      return "debug";
    default:
      break;
  }
  return "unknown";
}

#ifdef LOG

void
log_init(const nfc_context *context)
{
  nfc_rs_context_log_init(context);
}

void
log_exit(void)
{
  nfc_rs_context_log_exit();
}

void
log_put(const uint8_t group, const char *category, const uint8_t priority, const char *format, ...)
{
  va_list args;
  va_list args_copy;
  int rendered_length;
  size_t buffer_size;
  char *buffer;

  if (format == NULL) {
    log_dispatch_message(group, category, priority, LOG_FORMATTING_FAILED);
    return;
  }

  va_start(args, format);
  va_copy(args_copy, args);

#if defined(_WIN32)
  rendered_length = _vscprintf(format, args_copy);
#else
  rendered_length = vsnprintf(NULL, 0, format, args_copy);
#endif
  va_end(args_copy);

  if (rendered_length < 0) {
    va_end(args);
    log_dispatch_message(group, category, priority, LOG_FORMATTING_FAILED);
    return;
  }

  buffer_size = (size_t) rendered_length + 1u;
  buffer = (char *) malloc(buffer_size);
  if (buffer == NULL) {
    va_end(args);
    log_dispatch_message(group, category, priority, LOG_FORMATTING_FAILED);
    return;
  }

  if (vsnprintf(buffer, buffer_size, format, args) < 0) {
    free(buffer);
    va_end(args);
    log_dispatch_message(group, category, priority, LOG_FORMATTING_FAILED);
    return;
  }

  va_end(args);
  log_dispatch_message(group, category, priority, buffer);
  free(buffer);
}

void
log_put_message(uint8_t group, const char *category, uint8_t priority, const char *message)
{
  if (message == NULL) {
    message = "";
  }
  log_dispatch_message(group, category, priority, message);
}

#endif // LOG
