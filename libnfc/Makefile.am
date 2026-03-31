SUBDIRS = chips buses drivers .

# set the include path found by configure
AM_CPPFLAGS = $(all_includes) $(LIBNFC_CFLAGS) -DSYSCONFDIR='"$(sysconfdir)"'

lib_LTLIBRARIES = libnfc.la
libnfc_la_SOURCES = \
		    conf.c \
		    iso14443-subr.c \
		    mirror-subr.c \
		    nfc.c \
		    @USE_RUST_NFC_LIFECYCLE_FALSE@ \
		    nfc-emulation.c \
		    nfc-internal.c \
			@USE_RUST_NFC_SECURE_FALSE@ \
		    nfc-common.cpp \
		    target-subr.c \
		    target-subr-helpers.c \
		    target-subr-helpers2.c \
		    conf.h \
		    drivers.h \
		    iso7816.h \
		    log.h \
		    log-internal.h \
		    mirror-subr.h \
		    nfc-internal.h \
		    nfc-secure.h \
		    target-subr.h \
		    target-subr-internal.h

libnfc_la_LDFLAGS = -no-undefined -version-info 6:0:0 -export-symbols $(top_srcdir)/libnfc/libnfc.exports
libnfc_la_CFLAGS = @DRIVERS_CFLAGS@
libnfc_la_LIBADD = \
	$(top_builddir)/libnfc/chips/libnfcchips.la \
	$(top_builddir)/libnfc/buses/libnfcbuses.la \
	$(top_builddir)/libnfc/drivers/libnfcdrivers.la \
	$(top_builddir)/rust/target/release/liblibnfc_rs.a

if PCSC_ENABLED
  libnfc_la_CFLAGS += @libpcsclite_CFLAGS@ -DHAVE_PCSC
  libnfc_la_LIBADD += @libpcsclite_LIBS@
endif

if LIBUSB_ENABLED
  libnfc_la_CFLAGS += @libusb_CFLAGS@ -DHAVE_LIBUSB
  libnfc_la_LIBADD  += @libusb_LIBS@
endif

if WITH_LOG
	libnfc_la_SOURCES += log.cpp log-internal.c
endif

EXTRA_DIST = \
	CMakeLists.txt \
	additional-pages.dox \
	libnfc.exports
