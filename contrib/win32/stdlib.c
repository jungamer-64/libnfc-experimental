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
 * Copyright (C) 2013      Alex Lian
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
 *
 */

/**
 * @file stdlib.c
 * @brief Windows System compatibility
 */

// Handle platform specific includes
#include "contrib/windows.h"

// There is no setenv()and unsetenv() in windows,but we can use putenv() instead.
int setenv(const char *name, const char *value, int overwrite)
{
  if (!name || !value) {
    return -1;
  }

  char *env = getenv(name);
  if ((env && overwrite) || (!env)) {
    // Calculate required buffer size: name + "=" + value + null terminator
    size_t len = strlen(name) + strlen(value) + 2;
    char *str = malloc(len);
    if (!str) {
      return -1;
    }
    snprintf(str, len, "%s=%s", name, value);
    int result = putenv(str);
    // Note: Do not free str as putenv takes ownership of the string
    return result;
  }
  return -1;
}

void unsetenv(const char *name)
{
  if (!name) {
    return;
  }

  // Calculate required buffer size: name + "=" + null terminator
  size_t len = strlen(name) + 2;
  char *str = malloc(len);
  if (!str) {
    return;
  }
  snprintf(str, len, "%s=", name);
  putenv(str);
  // Note: Do not free str as putenv takes ownership of the string
}
