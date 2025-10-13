#cmakedefine PACKAGE_NAME "@PACKAGE_NAME@"
#cmakedefine PACKAGE_VERSION "@PACKAGE_VERSION@"
#cmakedefine PACKAGE_STRING "@PACKAGE_STRING@"
#cmakedefine SYSCONFDIR "@SYSCONFDIR@"

/* Request POSIX extensions when the toolchain has not already opted in. */
#ifndef _XOPEN_SOURCE
#cmakedefine _XOPEN_SOURCE @_XOPEN_SOURCE@
#endif
