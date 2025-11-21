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

#include <errno.h>
#include <stdlib.h>
#include <string.h>

// There is no setenv()and unsetenv() in windows,but we can use putenv() instead.
int setenv(const char *name, const char *value, int overwrite)
{
  if (!name || name[0] == '\0' || !value)
  {
    errno = EINVAL;
    return -1;
  }

  if (!overwrite && getenv(name))
  {
    return 0;
  }

#if defined(_MSC_VER) || defined(__MINGW32__) || defined(__MINGW64__)
  if (_putenv_s(name, value) != 0)
  {
    return -1;
  }
  return 0;
#else
  // Fallback to ANSI putenv if secure variant is unavailable
  size_t name_len = strlen(name);
  size_t value_len = strlen(value);
  size_t len = name_len + value_len + 2;
  char *str = malloc(len);
  if (!str)
  {
    errno = ENOMEM;
    return -1;
  }

  memcpy(str, name, name_len);
  str[name_len] = '=';
  memcpy(str + name_len + 1, value, value_len);
  str[len - 1] = '\0';

  if (putenv(str) != 0)
  {
    free(str);
    return -1;
  }
  return 0;
#endif
}

int unsetenv(const char *name)
{
  if (!name || name[0] == '\0')
  {
    errno = EINVAL;
    return -1;
  }

#if defined(_MSC_VER) || defined(__MINGW32__) || defined(__MINGW64__)
  if (_putenv_s(name, "") != 0)
  {
    return -1;
  }
  return 0;
#else
  size_t name_len = strlen(name);
  size_t len = name_len + 2;
  char *str = malloc(len);
  if (!str)
  {
    errno = ENOMEM;
    return -1;
  }

  memcpy(str, name, name_len);
  str[len - 2] = '=';
  str[len - 1] = '\0';

  if (putenv(str) != 0)
  {
    free(str);
    return -1;
  }
  return 0;
#endif
}
