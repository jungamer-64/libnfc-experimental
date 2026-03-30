#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <nfc/nfc.h>

int nfc_parse_connstring(const char *connstring,
                         const char *prefix,
                         const char *param_name,
                         char *param_value,
                         size_t param_value_size);

static char last_msg[1024];

void log_put_message(uint8_t group, const char *category, uint8_t priority, const char *message)
{
    (void)group;
    (void)category;
    (void)priority;
    if (message)
    {
        strncpy(last_msg, message, sizeof(last_msg) - 1);
        last_msg[sizeof(last_msg) - 1] = '\0';
    }
    else
    {
        last_msg[0] = '\0';
    }
}

int main(void)
{
    nfc_context *context = NULL;
    char buf[128];
    const char *conn = "pn53x_usb:/dev/usb";
    const char *prefix = "pn532"; // mismatching prefix to force debug message

    nfc_init(&context);
    if (!context)
    {
        fprintf(stderr, "Expected nfc_init to allocate a context\n");
        return 1;
    }
    nfc_exit(context);

    int rc = nfc_parse_connstring(conn, prefix, "param", buf, sizeof(buf));
    if (rc == 0)
    {
        fprintf(stderr, "Expected error due to prefix mismatch but got success\n");
        return 2;
    }

    const char *err = nfc_get_last_error();
    if (!err)
    {
        fprintf(stderr, "Expected last error to be set\n");
        return 3;
    }

    if (strstr(last_msg, "does not match prefix") == NULL)
    {
        fprintf(stderr, "Expected log message to contain 'does not match prefix' but got '%s'\n", last_msg);
        return 4;
    }

    printf("ffi-sanity OK: rc=%d last_error='%s' log='%s'\n", rc, err, last_msg);
    return 0;
}
