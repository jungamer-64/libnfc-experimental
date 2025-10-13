/**
 * @file nfc-common.h
 * @brief Common utility functions to reduce code duplication across drivers
 *
 * Refactored for maximum type safety and robustness:
 * - Enhanced const correctness
 * - Stronger type safety with opaque pointers
 * - Better null pointer handling
 * - Improved static inline function design
 * - More robust error handling patterns
 * - Memory leak prevention
 */

#ifndef __NFC_COMMON_H__
#define __NFC_COMMON_H__

#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <stdbool.h>
#include <stdint.h>

#ifndef WIN32
#include <unistd.h>
#endif

#include "nfc/nfc.h"
#include "nfc-internal.h"
#include "nfc-secure.h"
#include "log.h"

#ifdef __cplusplus
extern "C"
{
#endif

  /* ============================================================================
   * C STANDARD AND COMPILER DETECTION
   * ========================================================================== */

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L
#define NFC_COMMON_HAVE_C23 1
#else
#define NFC_COMMON_HAVE_C23 0
#endif

#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
#define NFC_COMMON_HAVE_C11 1
#else
#define NFC_COMMON_HAVE_C11 0
#endif

#if defined(__GNUC__) || defined(__clang__)
#define NFC_COMMON_HAVE_GNU_EXTENSIONS 1
#else
#define NFC_COMMON_HAVE_GNU_EXTENSIONS 0
#endif

  /* ============================================================================
   * ATTRIBUTES
   * ========================================================================== */

#if !defined(NFC_NODISCARD)
#if NFC_COMMON_HAVE_C23 && defined(__has_c_attribute)
#if __has_c_attribute(nodiscard)
#define NFC_NODISCARD [[nodiscard]]
#else
#define NFC_NODISCARD
#endif
#elif NFC_COMMON_HAVE_GNU_EXTENSIONS
#define NFC_NODISCARD __attribute__((warn_unused_result))
#else
#define NFC_NODISCARD
#endif
#endif

#if NFC_COMMON_HAVE_GNU_EXTENSIONS
#define NFC_NONNULL(...) __attribute__((nonnull(__VA_ARGS__)))
#define NFC_NONNULL_ALL __attribute__((nonnull))
#else
#define NFC_NONNULL(...)
#define NFC_NONNULL_ALL
#endif

#if NFC_COMMON_HAVE_GNU_EXTENSIONS
#define NFC_PURE __attribute__((pure))
#else
#define NFC_PURE
#endif

  /* ============================================================================
   * COMMON ERROR CODES
   * ========================================================================== */

  typedef enum
  {
    NFC_COMMON_SUCCESS = 0,
    NFC_COMMON_ERROR = -1,
    NFC_COMMON_NOMEM = -ENOMEM,
    NFC_COMMON_INVALID = -EINVAL,
    NFC_COMMON_EEXIST = -EEXIST
  } nfc_common_error_t;

  /* ============================================================================
   * LOGGING MACROS
   * ========================================================================== */

#define NFC_LOG_ERROR_AND_RETURN(error_code, format, ...)    \
  do                                                         \
  {                                                          \
    log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, \
            format, ##__VA_ARGS__);                          \
    return (error_code);                                     \
  } while (0)

#define NFC_LOG_ERROR(format, ...)                         \
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_ERROR, \
          format, ##__VA_ARGS__)

#define NFC_LOG_WARN(format, ...)                         \
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_WARN, \
          format, ##__VA_ARGS__)

#define NFC_LOG_INFO(format, ...)                         \
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_INFO, \
          format, ##__VA_ARGS__)

#define NFC_LOG_DEBUG(format, ...)                         \
  log_put(LOG_GROUP, LOG_CATEGORY, NFC_LOG_PRIORITY_DEBUG, \
          format, ##__VA_ARGS__)

  /* ============================================================================
   * TYPE-SAFE POINTER TYPES
   * ========================================================================== */

  /**
   * @brief Opaque port handle for type safety
   *
   * This incomplete type prevents accidental mixing of different port types
   * and enforces proper encapsulation.
   */
  typedef struct nfc_port_handle_s *nfc_port_handle_t;

  /**
   * @brief Port close function pointer type with type safety
   */
  typedef void (*nfc_port_close_fn)(nfc_port_handle_t port);

  /* ============================================================================
   * MEMORY MANAGEMENT HELPERS
   * ========================================================================== */

  /**
   * @brief Safe array cleanup helper
   *
   * Frees all elements in a NULL-terminated array, then frees the array itself.
   * Elements are set to NULL after freeing to prevent double-free.
   *
   * @param array NULL-terminated array of pointers to free
   *
   * @note Due to C's type system limitations, callers must cast to (void **)
   *       when passing typed pointer arrays (e.g., char **). This is a necessary
   *       compromise for generic pointer array handling in C.
   */
  static inline void
  nfc_free_array(void **array)
  {
    if (array == NULL)
    {
      return;
    }

    for (size_t i = 0; array[i] != NULL; i++)
    {
      free(array[i]);
      array[i] = NULL; /* Prevent double-free */
    }
    free(array);
  }

  /**
   * @brief Type-safe wrapper for freeing pointer arrays with nullification
   *
   * @param array_ptr Pointer to array pointer (will be set to NULL)
   *
   * @note This function safely handles the double pointer issue by accepting
   *       a pointer to the array pointer itself.
   */
  static inline void
  nfc_free_array_and_null(void ***array_ptr)
  {
    if (array_ptr == NULL || *array_ptr == NULL)
    {
      return;
    }

    nfc_free_array(*array_ptr);
    *array_ptr = NULL;
  }

  /**
   * @brief Device cleanup helper with return value
   *
   * @param ports NULL-terminated array of port strings to free
   * @param return_value Value to return after cleanup
   * @return The specified return_value
   */
  NFC_NODISCARD
  static inline int
  nfc_cleanup_and_return(void **ports, int return_value)
  {
    nfc_free_array(ports);
    return return_value;
  }

  /**
   * @brief Allocate and zero-initialize driver data with type safety
   *
   * This function prevents memory leaks by refusing to overwrite existing
   * driver_data allocations.
   *
   * @param pnd The device pointer whose driver_data will be allocated
   * @param data_size Size of the driver-specific data structure
   * @return 0 on success, negative error code on failure
   */
  NFC_NODISCARD NFC_NONNULL(1) static inline int nfc_alloc_driver_data(nfc_device *pnd, size_t data_size)
  {
    if (pnd == NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "NULL device pointer in driver data allocation");
      return NFC_COMMON_INVALID;
    }

    if (data_size == 0)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "Zero size for driver data allocation");
      return NFC_COMMON_INVALID;
    }

    /* Check for existing allocation - refuse to overwrite to prevent memory leak */
    if (pnd->driver_data != NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "Existing driver_data pointer found. Potential memory leak or "
              "double initialization. Refusing allocation.");
      return NFC_COMMON_EEXIST;
    }

    pnd->driver_data = calloc(1, data_size); /* Use calloc for zero-init */
    if (pnd->driver_data == NULL)
    {
      const int saved_errno = errno;
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "Failed to allocate driver data (%zu bytes): %s",
              data_size, strerror(saved_errno));
      return NFC_COMMON_NOMEM;
    }

    return NFC_COMMON_SUCCESS;
  }

  /**
   * @brief Allocate driver data or free and re-allocate if it exists
   *
   * This variant allows re-initialization by freeing existing data.
   * Use with caution - prefer nfc_alloc_driver_data() when possible.
   *
   * @param pnd The device pointer
   * @param data_size Size of the driver-specific data structure
   * @param free_fn Function to free existing driver_data (can be NULL for simple free)
   * @return 0 on success, negative error code on failure
   */
  NFC_NODISCARD NFC_NONNULL(1) static inline int nfc_realloc_driver_data(nfc_device *pnd, size_t data_size, void (*free_fn)(void *))
  {
    if (pnd == NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "NULL device pointer in driver data reallocation");
      return NFC_COMMON_INVALID;
    }

    /* Free existing data if present */
    if (pnd->driver_data != NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_WARN,
              "Freeing existing driver_data for reallocation");

      if (free_fn != NULL)
      {
        free_fn(pnd->driver_data);
      }
      else
      {
        free(pnd->driver_data);
      }
      pnd->driver_data = NULL;
    }

    return nfc_alloc_driver_data(pnd, data_size);
  }

  /* ============================================================================
   * DEVICE INITIALIZATION AND CLEANUP
   * ========================================================================== */

  /**
   * @brief Device initialization cleanup configuration
   */
  typedef struct
  {
    nfc_device *pnd;
    nfc_port_handle_t port;
    nfc_port_close_fn close_fn;
    void **ports;
    bool chip_data_allocated;
  } nfc_init_cleanup_t;

  /**
   * @brief Initialize cleanup configuration with safe defaults
   */
  static inline nfc_init_cleanup_t
  nfc_init_cleanup_new(void)
  {
    nfc_init_cleanup_t cleanup = {NULL, NULL, NULL, NULL, false};
    return cleanup;
  }

  /**
   * @brief Comprehensive device initialization error handler
   *
   * @param pnd Device pointer to clean up (can be NULL)
   * @param port Communication port to close (can be NULL)
   * @param close_fn Function pointer to close the port
   * @param ports Port array to free
   * @param chip_data_allocated Whether chip data was allocated
   * @return Always returns 0 for use in scan functions
   */
  NFC_NODISCARD
  int nfc_device_init_failed(nfc_device *pnd,
                             nfc_port_handle_t port,
                             nfc_port_close_fn close_fn,
                             void **ports,
                             bool chip_data_allocated);

  /**
   * @brief Structured device initialization cleanup
   *
   * @param cleanup Cleanup configuration structure
   * @return Always returns 0 for use in scan functions
   */
  NFC_NODISCARD
  static inline int
  nfc_device_init_failed_ex(const nfc_init_cleanup_t *cleanup)
  {
    if (cleanup == NULL)
    {
      return 0;
    }

    return nfc_device_init_failed(
        cleanup->pnd,
        cleanup->port,
        cleanup->close_fn,
        cleanup->ports,
        cleanup->chip_data_allocated);
  }

  /**
   * @brief Common resource cleanup for device open failures
   *
   * @param pnd Device to free (can be NULL)
   * @param driver_data Driver-specific data (freed if pnd is NULL)
   * @param chip_data_allocated Whether chip data needs freeing
   */
  void nfc_device_open_failed(nfc_device *pnd,
                              void *driver_data,
                              bool chip_data_allocated);

  /* ============================================================================
   * CONNECTION STRING HELPERS
   * ========================================================================== */

  /**
   * @brief Connection string parsing result
   */
  typedef struct
  {
    int status;          /* 0 on success, negative on error */
    size_t value_length; /* Length of extracted value */
  } nfc_connstring_result_t;

  /**
   * @brief Extract connection string components with enhanced validation
   *
   * @param connstring The connection string to parse
   * @param prefix Expected prefix (e.g., "pn53x_usb")
   * @param param_name Parameter name to extract
   * @param param_value Buffer to store extracted parameter value
   * @param param_value_size Size of param_value buffer
   * @return 0 on success, negative error code on failure
   */
  NFC_NODISCARD NFC_NONNULL(1, 2, 3, 4) int nfc_parse_connstring(const char *connstring,
                                                                 const char *prefix,
                                                                 const char *param_name,
                                                                 char *param_value,
                                                                 size_t param_value_size);

  /**
   * @brief Extended connstring parsing with result details
   *
   * @param connstring The connection string to parse
   * @param prefix Expected prefix
   * @param param_name Parameter name to extract
   * @param param_value Buffer to store extracted parameter value
   * @param param_value_size Size of param_value buffer
   * @return Parsing result with status and value length
   */
  NFC_NODISCARD NFC_NONNULL(1, 2, 3, 4) static inline nfc_connstring_result_t
      nfc_parse_connstring_ex(const char *connstring,
                              const char *prefix,
                              const char *param_name,
                              char *param_value,
                              size_t param_value_size)
  {
    nfc_connstring_result_t result;
    result.status = nfc_parse_connstring(connstring, prefix, param_name,
                                         param_value, param_value_size);
    result.value_length = 0;

    if (result.status == 0)
    {
      result.value_length = strlen(param_value);
    }

    return result;
  }

  /**
   * @brief Build standardized connection string
   *
   * @param dest Destination buffer
   * @param dest_size Size of destination buffer
   * @param driver_name Driver name
   * @param param_name Parameter name
   * @param param_value Parameter value
   * @return 0 on success, negative error code on failure
   */
  NFC_NODISCARD NFC_NONNULL_ALL int nfc_build_connstring(char *dest,
                                                         size_t dest_size,
                                                         const char *driver_name,
                                                         const char *param_name,
                                                         const char *param_value);

  /**
   * @brief Copy connection string safely with validation
   *
   * @param dest Destination connstring buffer
   * @param src Source connstring
   * @return 0 on success, negative error code on failure
   */
  NFC_NODISCARD NFC_NONNULL_ALL static inline int
  nfc_copy_connstring(nfc_connstring dest, const nfc_connstring src)
  {
    if (dest == NULL || src == NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "NULL pointer in connstring copy");
      return NFC_COMMON_INVALID;
    }

    const int result = nfc_safe_memcpy(dest, sizeof(nfc_connstring),
                                       src, sizeof(nfc_connstring));
    if (result < 0)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "Failed to copy connection string");
      return NFC_COMMON_ERROR;
    }

    return NFC_COMMON_SUCCESS;
  }

  /* ============================================================================
   * DEVICE VALIDATION
   * ========================================================================== */

  /**
   * @brief Validate device pointer before operations
   *
   * @param pnd Device pointer to validate
   * @param function_name Name of calling function (for error message)
   * @return true if valid, false if NULL
   */
  NFC_PURE
  static inline bool
  nfc_device_validate(const nfc_device *pnd, const char *function_name)
  {
    if (pnd == NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "%s: NULL device pointer",
              function_name != NULL ? function_name : "<unknown>");
      return false;
    }
    return true;
  }

  /**
   * @brief Validate device pointer and driver data
   *
   * @param pnd Device pointer to validate
   * @param function_name Name of calling function
   * @return true if both device and driver_data are valid
   */
  NFC_PURE
  static inline bool
  nfc_device_validate_with_data(const nfc_device *pnd, const char *function_name)
  {
    if (!nfc_device_validate(pnd, function_name))
    {
      return false;
    }

    if (pnd->driver_data == NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "%s: NULL driver_data pointer",
              function_name != NULL ? function_name : "<unknown>");
      return false;
    }

    return true;
  }

  /* ============================================================================
   * ABORT MECHANISM HELPERS (POSIX)
   * ========================================================================== */

#ifndef WIN32

  /**
   * @brief Abort mechanism file descriptor pair
   */
  typedef struct
  {
    int read_fd;
    int write_fd;
  } nfc_abort_fds_t;

  /**
   * @brief Initialize abort file descriptor pair with invalid values
   */
  static inline nfc_abort_fds_t
  nfc_abort_fds_init(void)
  {
    nfc_abort_fds_t fds = {-1, -1};
    return fds;
  }

  /**
   * @brief Check if abort fds are valid
   */
  NFC_PURE
  static inline bool
  nfc_abort_fds_valid(const nfc_abort_fds_t *fds)
  {
    return fds != NULL && fds->read_fd >= 0 && fds->write_fd >= 0;
  }

  /**
   * @brief Abort mechanism initialization helper (POSIX systems)
   *
   * @param abort_fds Array of 2 file descriptors for the pipe
   * @return 0 on success, negative error code on failure
   */
  NFC_NODISCARD NFC_NONNULL(1) static inline int nfc_init_abort_mechanism(int abort_fds[2])
  {
    if (abort_fds == NULL)
    {
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "NULL abort_fds pointer");
      return NFC_COMMON_INVALID;
    }

    if (pipe(abort_fds) < 0)
    {
      const int saved_errno = errno;
      log_put(NFC_LOG_GROUP_GENERAL, "libnfc.common", NFC_LOG_PRIORITY_ERROR,
              "Failed to create abort pipe: %s", strerror(saved_errno));
      return -saved_errno;
    }

    return NFC_COMMON_SUCCESS;
  }

  /**
   * @brief Structured abort mechanism initialization
   *
   * @param fds Pointer to abort file descriptor structure
   * @return 0 on success, negative error code on failure
   */
  NFC_NODISCARD NFC_NONNULL(1) static inline int nfc_init_abort_mechanism_ex(nfc_abort_fds_t *fds)
  {
    if (fds == NULL)
    {
      return NFC_COMMON_INVALID;
    }

    int pipe_fds[2];
    const int result = nfc_init_abort_mechanism(pipe_fds);

    if (result == 0)
    {
      fds->read_fd = pipe_fds[0];
      fds->write_fd = pipe_fds[1];
    }
    else
    {
      fds->read_fd = -1;
      fds->write_fd = -1;
    }

    return result;
  }

  /**
   * @brief Abort mechanism cleanup helper (POSIX systems)
   *
   * @param abort_fds Array of 2 file descriptors for the pipe
   */
  NFC_NONNULL(1)
  static inline void
  nfc_close_abort_mechanism(int abort_fds[2])
  {
    if (abort_fds == NULL)
    {
      return;
    }

    if (abort_fds[0] >= 0)
    {
      close(abort_fds[0]);
      abort_fds[0] = -1;
    }

    if (abort_fds[1] >= 0)
    {
      close(abort_fds[1]);
      abort_fds[1] = -1;
    }
  }

  /**
   * @brief Structured abort mechanism cleanup
   *
   * @param fds Pointer to abort file descriptor structure
   */
  NFC_NONNULL(1)
  static inline void
  nfc_close_abort_mechanism_ex(nfc_abort_fds_t *fds)
  {
    if (fds == NULL)
    {
      return;
    }

    if (fds->read_fd >= 0)
    {
      close(fds->read_fd);
      fds->read_fd = -1;
    }

    if (fds->write_fd >= 0)
    {
      close(fds->write_fd);
      fds->write_fd = -1;
    }
  }

#endif /* !WIN32 */

#ifdef __cplusplus
}
#endif

#endif /* __NFC_COMMON_H__ */
