// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Free/Libre Near Field Communication (NFC) library
//
// Public C-ABI compatibility helpers that remain part of libnfc's
// installed surface even after the core implementation moved to Rust.

use crate::bridge::{
    CStringOut, InputBytes, OutputBytes, baud_rate_from_c, free_rust_device, is_rust_shim_device,
    modulation_type_from_c,
};
use crate::ffi_strings::{baud_rate_label_cstr, modulation_label_cstr, version_cstr};
use crate::ffi_support::as_ref;
use crate::ffi_types::{
    nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_felica_info, nfc_iso14443a_info,
    nfc_iso14443b_info, nfc_iso14443b2ct_info, nfc_iso14443b2sr_info, nfc_iso14443bi_info,
    nfc_iso14443biclass_info, nfc_jewel_info, nfc_modulation_type, nfc_target,
};
use crate::lifecycle::nfc_device;
use crate::{
    ffi_catch_unwind_int, ffi_catch_unwind_ptr, ffi_catch_unwind_void, release_allocated_ptr,
};
use libc::{c_char, c_int, c_void, size_t};

#[cfg(test)]
use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use std::fmt::{self, Write as _};
use std::ptr;
#[cfg(test)]
use std::slice;

const NFC_ESOFT: c_int = -80;
const TARGET_RENDER_BUFFER_SIZE: usize = 4096;
const SAK_UID_NOT_COMPLETE: u8 = 0x04;
const SAK_ISO14443_4_COMPLIANT: u8 = 0x20;
const SAK_ISO18092_COMPLIANT: u8 = 0x40;
const PI_ISO14443_4_SUPPORTED: u8 = 0x01;
const PI_NAD_SUPPORTED: u8 = 0x01;
const PI_CID_SUPPORTED: u8 = 0x02;

struct CardAtqa {
    atqa: u16,
    mask: u16,
    type_name: &'static str,
    saklist: &'static [i32],
}

struct CardSak {
    sak: u8,
    mask: u8,
    type_name: &'static str,
}

const CARD_ATQAS: &[CardAtqa] = &[
    CardAtqa {
        atqa: 0x0044,
        mask: 0xffff,
        type_name: "MIFARE Ultralight",
        saklist: &[0, -1],
    },
    CardAtqa {
        atqa: 0x0044,
        mask: 0xffff,
        type_name: "MIFARE Ultralight C",
        saklist: &[0, -1],
    },
    CardAtqa {
        atqa: 0x0004,
        mask: 0xff0f,
        type_name: "MIFARE Mini 0.3K",
        saklist: &[1, -1],
    },
    CardAtqa {
        atqa: 0x0004,
        mask: 0xff0f,
        type_name: "MIFARE Classic 1K",
        saklist: &[2, -1],
    },
    CardAtqa {
        atqa: 0x0002,
        mask: 0xff0f,
        type_name: "MIFARE Classic 4K",
        saklist: &[3, -1],
    },
    CardAtqa {
        atqa: 0x0004,
        mask: 0xffff,
        type_name: "MIFARE Plus (4 Byte UID or 4 Byte RID)",
        saklist: &[4, 5, 6, 7, 8, 9, -1],
    },
    CardAtqa {
        atqa: 0x0002,
        mask: 0xffff,
        type_name: "MIFARE Plus (4 Byte UID or 4 Byte RID)",
        saklist: &[4, 5, 6, 7, 8, 9, -1],
    },
    CardAtqa {
        atqa: 0x0044,
        mask: 0xffff,
        type_name: "MIFARE Plus (7 Byte UID)",
        saklist: &[4, 5, 6, 7, 8, 9, -1],
    },
    CardAtqa {
        atqa: 0x0042,
        mask: 0xffff,
        type_name: "MIFARE Plus (7 Byte UID)",
        saklist: &[4, 5, 6, 7, 8, 9, -1],
    },
    CardAtqa {
        atqa: 0x0344,
        mask: 0xffff,
        type_name: "MIFARE DESFire",
        saklist: &[10, 11, -1],
    },
    CardAtqa {
        atqa: 0x0044,
        mask: 0xffff,
        type_name: "P3SR008",
        saklist: &[-1],
    },
    CardAtqa {
        atqa: 0x0004,
        mask: 0xf0ff,
        type_name: "SmartMX with MIFARE 1K emulation",
        saklist: &[12, -1],
    },
    CardAtqa {
        atqa: 0x0002,
        mask: 0xf0ff,
        type_name: "SmartMX with MIFARE 4K emulation",
        saklist: &[12, -1],
    },
    CardAtqa {
        atqa: 0x0048,
        mask: 0xf0ff,
        type_name: "SmartMX with 7 Byte UID",
        saklist: &[12, -1],
    },
];

const CARD_SAKS: &[CardSak] = &[
    CardSak {
        sak: 0x00,
        mask: 0xff,
        type_name: "",
    },
    CardSak {
        sak: 0x09,
        mask: 0xff,
        type_name: "",
    },
    CardSak {
        sak: 0x08,
        mask: 0xff,
        type_name: "",
    },
    CardSak {
        sak: 0x18,
        mask: 0xff,
        type_name: "",
    },
    CardSak {
        sak: 0x08,
        mask: 0xff,
        type_name: " 2K, Security level 1",
    },
    CardSak {
        sak: 0x18,
        mask: 0xff,
        type_name: " 4K, Security level 1",
    },
    CardSak {
        sak: 0x10,
        mask: 0xff,
        type_name: " 2K, Security level 2",
    },
    CardSak {
        sak: 0x11,
        mask: 0xff,
        type_name: " 4K, Security level 2",
    },
    CardSak {
        sak: 0x20,
        mask: 0xff,
        type_name: " 2K, Security level 3",
    },
    CardSak {
        sak: 0x20,
        mask: 0xff,
        type_name: " 4K, Security level 3",
    },
    CardSak {
        sak: 0x20,
        mask: 0xff,
        type_name: " 4K",
    },
    CardSak {
        sak: 0x20,
        mask: 0xff,
        type_name: " EV1 2K/4K/8K",
    },
    CardSak {
        sak: 0x00,
        mask: 0x00,
        type_name: "",
    },
];

macro_rules! read_unaligned_field {
    ($expr:expr) => {
        unsafe { ptr::addr_of!($expr).read_unaligned() }
    };
}

fn modulation_label(value: nfc_modulation_type) -> *const c_char {
    modulation_label_cstr(modulation_type_from_c(value)).as_ptr()
}

fn baud_rate_label(value: nfc_baud_rate) -> *const c_char {
    baud_rate_label_cstr(baud_rate_from_c(value)).as_ptr()
}

fn iso14443a_crc_bytes(data: &[u8]) -> [u8; 2] {
    let mut crc = 0x6363u16;
    for byte in data {
        let mut bt = *byte ^ (crc as u8);
        bt ^= bt << 4;
        crc = (crc >> 8) ^ (u16::from(bt) << 8) ^ (u16::from(bt) << 3) ^ (u16::from(bt) >> 4);
    }
    [(crc & 0xff) as u8, (crc >> 8) as u8]
}

fn iso14443b_crc_bytes(data: &[u8]) -> [u8; 2] {
    let mut crc = 0xffffu16;
    for byte in data {
        let mut bt = *byte ^ (crc as u8);
        bt ^= bt << 4;
        crc = (crc >> 8) ^ (u16::from(bt) << 8) ^ (u16::from(bt) << 3) ^ (u16::from(bt) >> 4);
    }
    crc = !crc;
    [(crc & 0xff) as u8, (crc >> 8) as u8]
}

fn locate_historical_bytes_offset(ats: &[u8]) -> Option<usize> {
    let t0 = *ats.first()?;
    let mut offset = 1usize;
    if t0 & 0x10 != 0 {
        offset += 1;
    }
    if t0 & 0x20 != 0 {
        offset += 1;
    }
    if t0 & 0x40 != 0 {
        offset += 1;
    }
    (offset < ats.len()).then_some(offset)
}

fn write_rendered(rendered: &mut String, args: fmt::Arguments<'_>) {
    rendered
        .write_fmt(args)
        .expect("rendering to a String should not fail");
}

fn truncate_rendered_bytes(rendered: &str, max_len: usize) -> &str {
    if rendered.len() <= max_len {
        return rendered;
    }

    let mut end = max_len;
    while !rendered.is_char_boundary(end) {
        end -= 1;
    }
    &rendered[..end]
}

fn write_hex(rendered: &mut String, bytes: &[u8]) {
    for byte in bytes {
        write_rendered(rendered, format_args!("{byte:02x}  "));
    }
    rendered.push('\n');
}

fn modulation_label_str(value: nfc_modulation_type) -> &'static str {
    modulation_label_cstr(modulation_type_from_c(value))
        .to_str()
        .expect("static modulation label should be utf-8")
}

fn baud_rate_label_str(value: nfc_baud_rate) -> &'static str {
    baud_rate_label_cstr(baud_rate_from_c(value))
        .to_str()
        .expect("static baud-rate label should be utf-8")
}

fn write_nfc_iso14443a_info(rendered: &mut String, info: nfc_iso14443a_info, verbose: bool) {
    let atqa = read_unaligned_field!(info.abtAtqa);
    let bt_sak = read_unaligned_field!(info.btSak);
    let uid = read_unaligned_field!(info.abtUid);
    let uid_len = read_unaligned_field!(info.szUidLen).min(uid.len());
    let ats = read_unaligned_field!(info.abtAts);
    let ats_len = read_unaligned_field!(info.szAtsLen).min(ats.len());
    let ats_slice = &ats[..ats_len];

    write_rendered(rendered, format_args!("    ATQA (SENS_RES): "));
    write_hex(rendered, &atqa);
    if verbose {
        write_rendered(rendered, format_args!("* UID size: "));
        match (atqa[1] & 0xc0) >> 6 {
            0 => rendered.push_str("single\n"),
            1 => rendered.push_str("double\n"),
            2 => rendered.push_str("triple\n"),
            _ => rendered.push_str("RFU\n"),
        }
        write_rendered(rendered, format_args!("* bit frame anticollision "));
        match atqa[1] & 0x1f {
            0x01 | 0x02 | 0x04 | 0x08 | 0x10 => rendered.push_str("supported\n"),
            _ => rendered.push_str("not supported\n"),
        }
    }

    write_rendered(
        rendered,
        format_args!(
            "       UID (NFCID{}): ",
            if uid.first().copied() == Some(0x08) {
                '3'
            } else {
                '1'
            }
        ),
    );
    write_hex(rendered, &uid[..uid_len]);
    if verbose && uid.first().copied() == Some(0x08) {
        rendered.push_str("* Random UID\n");
    }

    write_rendered(rendered, format_args!("      SAK (SEL_RES): "));
    write_hex(rendered, &[bt_sak]);
    if verbose {
        if bt_sak & SAK_UID_NOT_COMPLETE != 0 {
            rendered.push_str("* Warning! Cascade bit set: UID not complete\n");
        }
        rendered.push_str(if bt_sak & SAK_ISO14443_4_COMPLIANT != 0 {
            "* Compliant with ISO/IEC 14443-4\n"
        } else {
            "* Not compliant with ISO/IEC 14443-4\n"
        });
        rendered.push_str(if bt_sak & SAK_ISO18092_COMPLIANT != 0 {
            "* Compliant with ISO/IEC 18092\n"
        } else {
            "* Not compliant with ISO/IEC 18092\n"
        });
    }

    if !ats_slice.is_empty() {
        write_rendered(rendered, format_args!("                ATS: "));
        write_hex(rendered, ats_slice);
    }

    if !ats_slice.is_empty() && verbose {
        const MAX_FRAME_SIZES: [usize; 9] = [16, 24, 32, 40, 48, 64, 96, 128, 256];
        let mut offset = 1usize;

        write_rendered(
            rendered,
            format_args!(
                "* Max Frame Size accepted by PICC: {} bytes\n",
                MAX_FRAME_SIZES[(ats_slice[0] & 0x0f) as usize]
            ),
        );

        if ats_slice[0] & 0x10 != 0 && offset < ats_slice.len() {
            let ta = ats_slice[offset];
            offset += 1;
            rendered.push_str("* Bit Rate Capability:\n");
            if ta == 0 {
                rendered.push_str("  * PICC supports only 106 kbits/s in both directions\n");
            }
            if ta & (1 << 7) != 0 {
                rendered.push_str("  * Same bitrate in both directions mandatory\n");
            }
            if ta & (1 << 4) != 0 {
                rendered.push_str("  * PICC to PCD, DS=2, bitrate 212 kbits/s supported\n");
            }
            if ta & (1 << 5) != 0 {
                rendered.push_str("  * PICC to PCD, DS=4, bitrate 424 kbits/s supported\n");
            }
            if ta & (1 << 6) != 0 {
                rendered.push_str("  * PICC to PCD, DS=8, bitrate 847 kbits/s supported\n");
            }
            if ta & (1 << 0) != 0 {
                rendered.push_str("  * PCD to PICC, DR=2, bitrate 212 kbits/s supported\n");
            }
            if ta & (1 << 1) != 0 {
                rendered.push_str("  * PCD to PICC, DR=4, bitrate 424 kbits/s supported\n");
            }
            if ta & (1 << 2) != 0 {
                rendered.push_str("  * PCD to PICC, DR=8, bitrate 847 kbits/s supported\n");
            }
            if ta & (1 << 3) != 0 {
                rendered.push_str("  * ERROR unknown value\n");
            }
        }

        if ats_slice[0] & 0x20 != 0 && offset < ats_slice.len() {
            let tb = ats_slice[offset];
            offset += 1;
            write_rendered(
                rendered,
                format_args!(
                    "* Frame Waiting Time: {:.4} ms\n",
                    256.0 * 16.0 * (1 << ((tb & 0xf0) >> 4)) as f64 / 13560.0
                ),
            );
            if (tb & 0x0f) == 0 {
                rendered.push_str("* No Start-up Frame Guard Time required\n");
            } else {
                write_rendered(
                    rendered,
                    format_args!(
                        "* Start-up Frame Guard Time: {:.4} ms\n",
                        256.0 * 16.0 * (1 << (tb & 0x0f)) as f64 / 13560.0
                    ),
                );
            }
        }

        if ats_slice[0] & 0x40 != 0 && offset < ats_slice.len() {
            let tc = ats_slice[offset];
            offset += 1;
            rendered.push_str(if tc & 0x1 != 0 {
                "* Node Address supported\n"
            } else {
                "* Node Address not supported\n"
            });
            rendered.push_str(if tc & 0x2 != 0 {
                "* Card IDentifier supported\n"
            } else {
                "* Card IDentifier not supported\n"
            });
        }

        if ats_slice.len() > offset {
            write_rendered(rendered, format_args!("* Historical bytes Tk: "));
            write_hex(rendered, &ats_slice[offset..]);

            let cib = ats_slice[offset];
            offset += 1;
            if cib != 0x00 && cib != 0x10 && (cib & 0xf0) != 0x80 {
                rendered.push_str("  * Proprietary format\n");
                if cib == 0xc1 {
                    rendered.push_str("    * Tag byte: Mifare or virtual cards of various types\n");
                    if offset < ats_slice.len() {
                        let l = ats_slice[offset];
                        offset += 1;
                        if l as usize != (ats_slice.len().saturating_sub(offset)) {
                            write_rendered(
                                rendered,
                                format_args!(
                                    "    * Warning: Type Identification Coding length ({l}) not matching Tk length ({})\n",
                                    ats_slice.len().saturating_sub(offset)
                                ),
                            );
                        }
                    }
                    if ats_slice.len().saturating_sub(offset) > 2 {
                        let ctc = ats_slice[offset];
                        offset += 1;
                        rendered.push_str("    * Chip Type: ");
                        rendered.push_str(match ctc & 0xf0 {
                            0x00 => "(Multiple) Virtual Cards\n",
                            0x10 => "Mifare DESFire\n",
                            0x20 => "Mifare Plus\n",
                            _ => "RFU\n",
                        });
                        rendered.push_str("    * Memory size: ");
                        rendered.push_str(match ctc & 0x0f {
                            0x00 => "<1 kbyte\n",
                            0x01 => "1 kbyte\n",
                            0x02 => "2 kbyte\n",
                            0x03 => "4 kbyte\n",
                            0x04 => "8 kbyte\n",
                            0x0f => "Unspecified\n",
                            _ => "RFU\n",
                        });
                    }
                    if !ats_slice[offset..].is_empty() {
                        let cvc = ats_slice[offset];
                        offset += 1;
                        rendered.push_str("    * Chip Status: ");
                        rendered.push_str(match cvc & 0xf0 {
                            0x00 => "Engineering sample\n",
                            0x20 => "Released\n",
                            _ => "RFU\n",
                        });
                        rendered.push_str("    * Chip Generation: ");
                        rendered.push_str(match cvc & 0x0f {
                            0x00 => "Generation 1\n",
                            0x01 => "Generation 2\n",
                            0x02 => "Generation 3\n",
                            0x0f => "Unspecified\n",
                            _ => "RFU\n",
                        });
                    }
                    if !ats_slice[offset..].is_empty() {
                        let vcs = ats_slice[offset];
                        rendered.push_str("    * Specifics (Virtual Card Selection):\n");
                        match vcs & 0x09 {
                            0x00 => rendered.push_str("      * Only VCSL supported\n"),
                            0x01 => rendered.push_str("      * VCS, VCSL and SVC supported\n"),
                            _ => {}
                        }
                        match vcs & 0x0f {
                            value if (value & 0x0e) == 0x00 => {
                                rendered.push_str("      * SL1, SL2(?), SL3 supported\n")
                            }
                            value if (value & 0x0e) == 0x02 => {
                                rendered.push_str("      * SL3 only card\n")
                            }
                            0x0e => rendered.push_str("      * No VCS command supported\n"),
                            0x0f => rendered.push_str("      * Unspecified\n"),
                            _ => rendered.push_str("      * RFU\n"),
                        }
                    }
                }
            } else if cib == 0x00 {
                rendered.push_str(
                    "  * Tk after 0x00 consist of optional consecutive COMPACT-TLV data objects\n\
    followed by a mandatory status indicator (the last three bytes, not in TLV)\n\
    See ISO/IEC 7816-4 8.1.1.3 for more info\n",
                );
            } else if cib == 0x10 {
                if offset < ats_slice.len() {
                    write_rendered(
                        rendered,
                        format_args!("  * DIR data reference: {:02x}\n", ats_slice[offset]),
                    );
                }
            } else if cib == 0x80 {
                if ats_slice.len() == offset {
                    rendered.push_str("  * No COMPACT-TLV objects found, no status found\n");
                } else {
                    rendered.push_str(
                        "  * Tk after 0x80 consist of optional consecutive COMPACT-TLV data objects;\n\
    the last data object may carry a status indicator of one, two or three bytes.\n\
    See ISO/IEC 7816-4 8.1.1.3 for more info\n",
                    );
                }
            }
        }
    }

    if verbose {
        let atqa_value = ((atqa[0] as u16) << 8) + atqa[1] as u16;
        let mut found_possible_match = false;
        let atqasak = ((atqa[0] as u32) << 16) + ((atqa[1] as u32) << 8) + bt_sak as u32;

        rendered.push_str("\nFingerprinting based on MIFARE type Identification Procedure:\n");
        for card in CARD_ATQAS {
            if (atqa_value & card.mask) == card.atqa {
                for sak_index in card.saklist.iter().copied().take_while(|value| *value >= 0) {
                    let sak = &CARD_SAKS[sak_index as usize];
                    if (bt_sak & sak.mask) == sak.sak {
                        write_rendered(
                            rendered,
                            format_args!("* {}{}\n", card.type_name, sak.type_name),
                        );
                        found_possible_match = true;
                    }
                }
            }
        }

        rendered.push_str("Other possible matches based on ATQA & SAK values:\n");
        match atqasak {
            0x000488 => {
                rendered.push_str("* Mifare Classic 1K Infineon\n");
                found_possible_match = true;
            }
            0x000298 => {
                rendered.push_str("* Gemplus MPCOS\n");
                found_possible_match = true;
            }
            0x030428 => {
                rendered.push_str("* JCOP31\n");
                found_possible_match = true;
            }
            0x004820 => {
                rendered.push_str("* JCOP31 v2.4.1\n");
                rendered.push_str("* JCOP31 v2.2\n");
                found_possible_match = true;
            }
            0x000428 => {
                rendered.push_str("* JCOP31 v2.3.1\n");
                found_possible_match = true;
            }
            0x000453 => {
                rendered.push_str("* Fudan FM1208SH01\n");
                found_possible_match = true;
            }
            0x000820 => {
                rendered.push_str("* Fudan FM1208\n");
                found_possible_match = true;
            }
            0x000238 => {
                rendered.push_str("* MFC 4K emulated by Nokia 6212 Classic\n");
                found_possible_match = true;
            }
            0x000838 => {
                rendered.push_str("* MFC 4K emulated by Nokia 6131 NFC\n");
                found_possible_match = true;
            }
            _ => {}
        }
        if !found_possible_match {
            rendered.push_str("* Unknown card, sorry\n");
        }
    }
}

fn write_nfc_felica_info(rendered: &mut String, info: nfc_felica_info) {
    let id = read_unaligned_field!(info.abtId);
    let pad = read_unaligned_field!(info.abtPad);
    let sys_code = read_unaligned_field!(info.abtSysCode);
    write_rendered(rendered, format_args!("        ID (NFCID2): "));
    write_hex(rendered, &id);
    write_rendered(rendered, format_args!("    Parameter (PAD): "));
    write_hex(rendered, &pad);
    write_rendered(rendered, format_args!("   System Code (SC): "));
    write_hex(rendered, &sys_code);
}

fn write_nfc_jewel_info(rendered: &mut String, info: nfc_jewel_info) {
    let sens_res = read_unaligned_field!(info.btSensRes);
    let id = read_unaligned_field!(info.btId);
    write_rendered(rendered, format_args!("    ATQA (SENS_RES): "));
    write_hex(rendered, &sens_res);
    write_rendered(rendered, format_args!("      4-LSB JEWELID: "));
    write_hex(rendered, &id);
}

fn write_nfc_barcode_info(rendered: &mut String, info: crate::ffi_types::nfc_barcode_info) {
    let data = read_unaligned_field!(info.abtData);
    let data_len = read_unaligned_field!(info.szDataLen).min(data.len());
    write_rendered(
        rendered,
        format_args!("        Size (bits): {}\n", data_len * 8),
    );
    rendered.push_str("            Content: ");
    for (index, byte) in data[..data_len].iter().enumerate() {
        write_rendered(rendered, format_args!("{byte:02X}"));
        if (index % 8 == 7) && (index < data_len.saturating_sub(1)) {
            rendered.push_str("\n                     ");
        }
    }
    rendered.push('\n');
}

fn write_nfc_iso14443b_info(rendered: &mut String, info: nfc_iso14443b_info, verbose: bool) {
    let pupi = read_unaligned_field!(info.abtPupi);
    let application_data = read_unaligned_field!(info.abtApplicationData);
    let protocol_info = read_unaligned_field!(info.abtProtocolInfo);
    write_rendered(rendered, format_args!("               PUPI: "));
    write_hex(rendered, &pupi);
    write_rendered(rendered, format_args!("   Application Data: "));
    write_hex(rendered, &application_data);
    write_rendered(rendered, format_args!("      Protocol Info: "));
    write_hex(rendered, &protocol_info);
    if verbose {
        const MAX_FRAME_SIZES: [usize; 9] = [16, 24, 32, 40, 48, 64, 96, 128, 256];
        rendered.push_str("* Bit Rate Capability:\n");
        if protocol_info[0] == 0 {
            rendered.push_str(" * PICC supports only 106 kbits/s in both directions\n");
        }
        if protocol_info[0] & (1 << 7) != 0 {
            rendered.push_str(" * Same bitrate in both directions mandatory\n");
        }
        if protocol_info[0] & (1 << 4) != 0 {
            rendered.push_str(" * PICC to PCD, 1etu=64/fc, bitrate 212 kbits/s supported\n");
        }
        if protocol_info[0] & (1 << 5) != 0 {
            rendered.push_str(" * PICC to PCD, 1etu=32/fc, bitrate 424 kbits/s supported\n");
        }
        if protocol_info[0] & (1 << 6) != 0 {
            rendered.push_str(" * PICC to PCD, 1etu=16/fc, bitrate 847 kbits/s supported\n");
        }
        if protocol_info[0] & (1 << 0) != 0 {
            rendered.push_str(" * PCD to PICC, 1etu=64/fc, bitrate 212 kbits/s supported\n");
        }
        if protocol_info[0] & (1 << 1) != 0 {
            rendered.push_str(" * PCD to PICC, 1etu=32/fc, bitrate 424 kbits/s supported\n");
        }
        if protocol_info[0] & (1 << 2) != 0 {
            rendered.push_str(" * PCD to PICC, 1etu=16/fc, bitrate 847 kbits/s supported\n");
        }
        if protocol_info[0] & (1 << 3) != 0 {
            rendered.push_str(" * ERROR unknown value\n");
        }
        if (protocol_info[1] & 0xf0) <= 0x80 {
            write_rendered(
                rendered,
                format_args!(
                    "* Maximum frame sizes: {} bytes\n",
                    MAX_FRAME_SIZES[((protocol_info[1] & 0xf0) >> 4) as usize]
                ),
            );
        }
        if (protocol_info[1] & 0x01) == PI_ISO14443_4_SUPPORTED {
            rendered.push_str("* Protocol types supported: ISO/IEC 14443-4\n");
        }
        write_rendered(
            rendered,
            format_args!(
                "* Frame Waiting Time: {:.4} ms\n",
                256.0 * 16.0 * (1 << ((protocol_info[2] & 0xf0) >> 4)) as f64 / 13560.0
            ),
        );
        if protocol_info[2] & (PI_NAD_SUPPORTED | PI_CID_SUPPORTED) != 0 {
            rendered.push_str("* Frame options supported: ");
            if protocol_info[2] & PI_NAD_SUPPORTED != 0 {
                rendered.push_str("NAD ");
            }
            if protocol_info[2] & PI_CID_SUPPORTED != 0 {
                rendered.push_str("CID ");
            }
            rendered.push('\n');
        }
    }
}

fn write_nfc_iso14443bi_info(rendered: &mut String, info: nfc_iso14443bi_info, verbose: bool) {
    let div = read_unaligned_field!(info.abtDIV);
    let ver_log = read_unaligned_field!(info.btVerLog);
    let config = read_unaligned_field!(info.btConfig);
    let atr = read_unaligned_field!(info.abtAtr);
    let atr_len = read_unaligned_field!(info.szAtrLen).min(atr.len());

    write_rendered(rendered, format_args!("                DIV: "));
    write_hex(rendered, &div);
    if verbose {
        let version = (ver_log & 0x1e) >> 1;
        rendered.push_str("   Software Version: ");
        if version == 15 {
            rendered.push_str("Undefined\n");
        } else {
            write_rendered(rendered, format_args!("{version}\n"));
        }
        if (ver_log & 0x80) != 0 && (config & 0x80) != 0 {
            rendered.push_str("        Wait Enable: yes");
        }
    }
    if (ver_log & 0x80) != 0 && (config & 0x40) != 0 {
        rendered.push_str("                ATS: ");
        write_hex(rendered, &atr[..atr_len]);
    }
}

fn write_simple_uid(rendered: &mut String, label: &str, uid: &[u8]) {
    write_rendered(rendered, format_args!("{label}"));
    write_hex(rendered, uid);
}

fn write_nfc_iso14443b2ct_info(rendered: &mut String, info: nfc_iso14443b2ct_info) {
    let uid = read_unaligned_field!(info.abtUID);
    let prod_code = read_unaligned_field!(info.btProdCode);
    let fab_code = read_unaligned_field!(info.btFabCode);
    let uid_decimal =
        ((uid[3] as u32) << 24) + ((uid[2] as u32) << 16) + ((uid[1] as u32) << 8) + uid[0] as u32;
    write_simple_uid(rendered, "                UID: ", &uid);
    write_rendered(
        rendered,
        format_args!("      UID (decimal): {uid_decimal:010}\n"),
    );
    write_rendered(
        rendered,
        format_args!("       Product Code: {prod_code:02X}\n"),
    );
    write_rendered(
        rendered,
        format_args!("           Fab Code: {fab_code:02X}\n"),
    );
}

fn write_nfc_dep_info(rendered: &mut String, info: nfc_dep_info) {
    let nfcid3 = read_unaligned_field!(info.abtNFCID3);
    let bs = read_unaligned_field!(info.btBS);
    let br = read_unaligned_field!(info.btBR);
    let timeout = read_unaligned_field!(info.btTO);
    let pp = read_unaligned_field!(info.btPP);
    let general_bytes = read_unaligned_field!(info.abtGB);
    let general_bytes_len = read_unaligned_field!(info.szGB).min(general_bytes.len());

    write_rendered(rendered, format_args!("       NFCID3: "));
    write_hex(rendered, &nfcid3);
    write_rendered(rendered, format_args!("           BS: {bs:02x}\n"));
    write_rendered(rendered, format_args!("           BR: {br:02x}\n"));
    write_rendered(rendered, format_args!("           TO: {timeout:02x}\n"));
    write_rendered(rendered, format_args!("           PP: {pp:02x}\n"));
    if general_bytes_len > 0 {
        write_rendered(rendered, format_args!("General Bytes: "));
        write_hex(rendered, &general_bytes[..general_bytes_len]);
    }
}

fn render_nfc_target(target: *const nfc_target, verbose: bool) -> String {
    let mut rendered = String::new();
    if target.is_null() {
        return rendered;
    }

    let target_ref = unsafe { &*target };
    let modulation_type = read_unaligned_field!(target_ref.nm.nmt);
    let baud_rate = read_unaligned_field!(target_ref.nm.nbr);
    let dep_suffix = if modulation_type == nfc_modulation_type::NMT_DEP {
        let dep_info = read_unaligned_field!(target_ref.nti.ndi);
        if read_unaligned_field!(dep_info.ndm) == nfc_dep_mode::NDM_ACTIVE {
            "active mode"
        } else {
            "passive mode"
        }
    } else {
        ""
    };

    write_rendered(
        &mut rendered,
        format_args!(
            "{} ({}{}) target:\n",
            modulation_label_str(modulation_type),
            baud_rate_label_str(baud_rate),
            dep_suffix
        ),
    );

    match modulation_type {
        nfc_modulation_type::NMT_ISO14443A => {
            write_nfc_iso14443a_info(
                &mut rendered,
                read_unaligned_field!(target_ref.nti.nai),
                verbose,
            );
        }
        nfc_modulation_type::NMT_JEWEL => {
            write_nfc_jewel_info(&mut rendered, read_unaligned_field!(target_ref.nti.nji));
        }
        nfc_modulation_type::NMT_BARCODE => {
            write_nfc_barcode_info(&mut rendered, read_unaligned_field!(target_ref.nti.nti));
        }
        nfc_modulation_type::NMT_FELICA => {
            write_nfc_felica_info(&mut rendered, read_unaligned_field!(target_ref.nti.nfi));
        }
        nfc_modulation_type::NMT_ISO14443B => {
            write_nfc_iso14443b_info(
                &mut rendered,
                read_unaligned_field!(target_ref.nti.nbi),
                verbose,
            );
        }
        nfc_modulation_type::NMT_ISO14443BI => {
            write_nfc_iso14443bi_info(
                &mut rendered,
                read_unaligned_field!(target_ref.nti.nii),
                verbose,
            );
        }
        nfc_modulation_type::NMT_ISO14443B2SR => {
            let info: nfc_iso14443b2sr_info = read_unaligned_field!(target_ref.nti.nsi);
            write_simple_uid(
                &mut rendered,
                "                UID: ",
                &read_unaligned_field!(info.abtUID),
            );
        }
        nfc_modulation_type::NMT_ISO14443BICLASS => {
            let info: nfc_iso14443biclass_info = read_unaligned_field!(target_ref.nti.nhi);
            write_simple_uid(
                &mut rendered,
                "                UID: ",
                &read_unaligned_field!(info.abtUID),
            );
        }
        nfc_modulation_type::NMT_ISO14443B2CT => {
            write_nfc_iso14443b2ct_info(&mut rendered, read_unaligned_field!(target_ref.nti.nci));
        }
        nfc_modulation_type::NMT_DEP => {
            write_nfc_dep_info(&mut rendered, read_unaligned_field!(target_ref.nti.ndi));
        }
        nfc_modulation_type::NMT_UNDEFINED => {}
    }

    rendered
}

pub unsafe fn nfc_close(device: *mut nfc_device) {
    ffi_catch_unwind_void("nfc_close", || unsafe {
        if is_rust_shim_device(device) {
            free_rust_device(device);
            return;
        }

        let Some(device_ref) = as_ref(device) else {
            return;
        };
        let Some(driver_ref) = as_ref(device_ref.driver) else {
            return;
        };
        if let Some(close) = driver_ref.close {
            close(device);
        }
    });
}

pub unsafe fn nfc_free(ptr: *mut c_void) {
    ffi_catch_unwind_void("nfc_free", || unsafe {
        release_allocated_ptr(ptr);
    });
}

pub unsafe fn nfc_version() -> *const c_char {
    ffi_catch_unwind_ptr("nfc_version", || version_cstr().as_ptr().cast_mut()) as *const c_char
}

pub unsafe fn str_nfc_baud_rate(value: nfc_baud_rate) -> *const c_char {
    ffi_catch_unwind_ptr("str_nfc_baud_rate", || baud_rate_label(value).cast_mut()) as *const c_char
}

pub unsafe fn str_nfc_modulation_type(value: nfc_modulation_type) -> *const c_char {
    ffi_catch_unwind_ptr("str_nfc_modulation_type", || {
        modulation_label(value).cast_mut()
    }) as *const c_char
}

pub unsafe fn str_nfc_target(
    buf: *mut *mut c_char,
    target: *const nfc_target,
    verbose: bool,
) -> c_int {
    ffi_catch_unwind_int("str_nfc_target", NFC_ESOFT, || unsafe {
        let output = match CStringOut::from_raw(ptr::null_mut(), buf) {
            Ok(output) => output,
            Err(status) => return status,
        };
        let rendered_text = render_nfc_target(target, verbose);
        output.write_back(
            ptr::null_mut(),
            truncate_rendered_bytes(&rendered_text, TARGET_RENDER_BUFFER_SIZE.saturating_sub(1)),
        )
    })
}

pub unsafe fn iso14443a_crc(data: *mut u8, len: size_t, crc: *mut u8) {
    ffi_catch_unwind_void("iso14443a_crc", || unsafe {
        if data.is_null() || crc.is_null() {
            return;
        }
        let bytes = match InputBytes::from_raw(ptr::null_mut(), data.cast_const(), len) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let mut crc_out = match OutputBytes::from_raw(ptr::null_mut(), crc, 2) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let out = iso14443a_crc_bytes(bytes.as_slice());
        let buffer = crc_out.as_mut_slice();
        if buffer.len() < out.len() {
            return;
        }
        buffer[..out.len()].copy_from_slice(&out);
    });
}

pub unsafe fn iso14443a_crc_append(data: *mut u8, len: size_t) {
    ffi_catch_unwind_void("iso14443a_crc_append", || unsafe {
        if data.is_null() {
            return;
        }
        let bytes = match InputBytes::from_raw(ptr::null_mut(), data.cast_const(), len) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let out = iso14443a_crc_bytes(bytes.as_slice());
        *data.add(len) = out[0];
        *data.add(len + 1) = out[1];
    });
}

pub unsafe fn iso14443b_crc(data: *mut u8, len: size_t, crc: *mut u8) {
    ffi_catch_unwind_void("iso14443b_crc", || unsafe {
        if data.is_null() || crc.is_null() {
            return;
        }
        let bytes = match InputBytes::from_raw(ptr::null_mut(), data.cast_const(), len) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let mut crc_out = match OutputBytes::from_raw(ptr::null_mut(), crc, 2) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let out = iso14443b_crc_bytes(bytes.as_slice());
        let buffer = crc_out.as_mut_slice();
        if buffer.len() < out.len() {
            return;
        }
        buffer[..out.len()].copy_from_slice(&out);
    });
}

pub unsafe fn iso14443b_crc_append(data: *mut u8, len: size_t) {
    ffi_catch_unwind_void("iso14443b_crc_append", || unsafe {
        if data.is_null() {
            return;
        }
        let bytes = match InputBytes::from_raw(ptr::null_mut(), data.cast_const(), len) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let out = iso14443b_crc_bytes(bytes.as_slice());
        *data.add(len) = out[0];
        *data.add(len + 1) = out[1];
    });
}

pub unsafe fn iso14443a_locate_historical_bytes(
    ats: *mut u8,
    ats_len: size_t,
    tk_len: *mut size_t,
) -> *mut u8 {
    ffi_catch_unwind_ptr("iso14443a_locate_historical_bytes", || unsafe {
        if !tk_len.is_null() {
            *tk_len = 0;
        }
        if ats.is_null() {
            return ptr::null_mut();
        }

        let ats_slice = match InputBytes::from_raw(ptr::null_mut(), ats.cast_const(), ats_len) {
            Ok(bytes) => bytes,
            Err(_) => return ptr::null_mut(),
        };
        let Some(offset) = locate_historical_bytes_offset(ats_slice.as_slice()) else {
            return ptr::null_mut();
        };
        if !tk_len.is_null() {
            *tk_len = ats_len - offset;
        }
        ats.add(offset).cast()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi_types::{
        nfc_dep_mode, nfc_felica_info, nfc_iso14443a_info, nfc_target, nfc_target_info,
    };
    use crate::lifecycle::nfc_driver;
    use std::ffi::CStr;

    fn render_target(target: *const nfc_target, verbose: bool) -> (c_int, *mut c_char, String) {
        let mut rendered = ptr::null_mut();
        let len = unsafe { str_nfc_target(ptr::addr_of_mut!(rendered), target, verbose) };
        let text = unsafe { CStr::from_ptr(rendered) }
            .to_string_lossy()
            .into_owned();
        (len, rendered, text)
    }

    fn iso14443a_target() -> nfc_target {
        nfc_target {
            nm: crate::ffi_types::nfc_modulation {
                nmt: nfc_modulation_type::NMT_ISO14443A,
                nbr: nfc_baud_rate::NBR_106,
            },
            nti: nfc_target_info {
                nai: nfc_iso14443a_info {
                    abtAtqa: [0x00, 0x04],
                    btSak: 0x08,
                    szUidLen: 4,
                    abtUid: [0x01, 0x23, 0x45, 0x67, 0, 0, 0, 0, 0, 0],
                    szAtsLen: 0,
                    abtAts: [0; 254],
                },
            },
        }
    }

    unsafe extern "C" fn test_close(device: *mut nfc_device) {
        unsafe {
            (*device).last_error = 123;
        }
    }

    #[test]
    fn version_is_non_empty() {
        let version = unsafe { CStr::from_ptr(nfc_version()) }.to_str().unwrap();
        assert!(!version.is_empty());
    }

    #[test]
    fn close_dispatches_driver_callback() {
        let driver = nfc_driver {
            name: ptr::null(),
            scan_type: crate::lifecycle::scan_type_enum::NOT_AVAILABLE,
            scan: None,
            open: None,
            close: Some(test_close),
            strerror: None,
            initiator_init: None,
            initiator_init_secure_element: None,
            initiator_select_passive_target: None,
            initiator_poll_target: None,
            initiator_select_dep_target: None,
            initiator_deselect_target: None,
            initiator_transceive_bytes: None,
            initiator_transceive_bits: None,
            initiator_transceive_bytes_timed: None,
            initiator_transceive_bits_timed: None,
            initiator_target_is_present: None,
            target_init: None,
            target_send_bytes: None,
            target_receive_bytes: None,
            target_send_bits: None,
            target_receive_bits: None,
            device_set_property_bool: None,
            device_set_property_int: None,
            get_supported_modulation: None,
            get_supported_baud_rate: None,
            device_get_information_about: None,
            abort_command: None,
            idle: None,
            powerdown: None,
        };
        let mut device = nfc_device {
            context: ptr::null(),
            driver: ptr::addr_of!(driver),
            driver_data: ptr::null_mut(),
            chip_data: ptr::null_mut(),
            name: [0; crate::lifecycle::DEVICE_NAME_LENGTH],
            connstring: [0; NFC_BUFSIZE_CONNSTRING],
            bCrc: false,
            bPar: false,
            bEasyFraming: false,
            bInfiniteSelect: false,
            bAutoIso14443_4: false,
            btSupportByte: 0,
            last_error: 0,
        };

        unsafe { nfc_close(ptr::addr_of_mut!(device)) };
        assert_eq!(device.last_error, 123);
    }

    #[test]
    fn target_renderer_allows_null_target_and_returns_empty_string() {
        let (len, rendered, text) = render_target(ptr::null(), false);

        assert_eq!(len, 0);
        assert!(!rendered.is_null());
        assert!(text.is_empty());

        unsafe { nfc_free(rendered.cast()) };
    }

    #[test]
    fn target_renderer_formats_verbose_iso14443a_details() {
        let target = iso14443a_target();
        let (len, rendered, text) = render_target(ptr::addr_of!(target), true);

        assert!(len > 0);
        assert!(text.starts_with("ISO/IEC 14443A (106 kbps) target:\n"));
        assert!(text.contains("ATQA (SENS_RES): 00  04  \n"));
        assert!(text.contains("UID (NFCID1): 01  23  45  67  \n"));
        assert!(text.contains("SAK (SEL_RES): 08  \n"));
        assert!(text.contains("* UID size: single\n"));
        assert!(text.contains("Fingerprinting based on MIFARE type Identification Procedure:\n"));

        unsafe { nfc_free(rendered.cast()) };
    }

    #[test]
    fn target_renderer_includes_dep_mode_suffix() {
        let target = nfc_target {
            nm: crate::ffi_types::nfc_modulation {
                nmt: nfc_modulation_type::NMT_DEP,
                nbr: nfc_baud_rate::NBR_106,
            },
            nti: nfc_target_info {
                ndi: crate::ffi_types::nfc_dep_info {
                    abtNFCID3: [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a],
                    btDID: 0,
                    btBS: 0x11,
                    btBR: 0x22,
                    btTO: 0x33,
                    btPP: 0x44,
                    abtGB: [0; 48],
                    szGB: 0,
                    ndm: nfc_dep_mode::NDM_ACTIVE,
                },
            },
        };
        let (len, rendered, text) = render_target(ptr::addr_of!(target), false);

        assert!(len > 0);
        assert!(text.starts_with("D.E.P. (106 kbpsactive mode) target:\n"));
        assert!(text.contains("NFCID3: 01  02  03  04  05  06  07  08  09  0a  \n"));

        unsafe { nfc_free(rendered.cast()) };
    }

    #[test]
    fn target_renderer_formats_non_iso14443a_targets() {
        let target = nfc_target {
            nm: crate::ffi_types::nfc_modulation {
                nmt: nfc_modulation_type::NMT_FELICA,
                nbr: nfc_baud_rate::NBR_212,
            },
            nti: nfc_target_info {
                nfi: nfc_felica_info {
                    szLen: 18,
                    btResCode: 0x01,
                    abtId: [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef],
                    abtPad: [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe],
                    abtSysCode: [0x12, 0x34],
                },
            },
        };
        let (len, rendered, text) = render_target(ptr::addr_of!(target), false);

        assert!(len > 0);
        assert!(text.starts_with("FeliCa (212 kbps) target:\n"));
        assert!(text.contains("ID (NFCID2): 01  23  45  67  89  ab  cd  ef  \n"));
        assert!(text.contains("Parameter (PAD): 10  32  54  76  98  ba  dc  fe  \n"));
        assert!(text.contains("System Code (SC): 12  34  \n"));

        unsafe { nfc_free(rendered.cast()) };
    }

    #[test]
    fn iso14443_crc_helpers_match_known_values() {
        let mut atqa = [0x26u8];
        let mut a_crc = [0u8; 2];
        unsafe { iso14443a_crc(atqa.as_mut_ptr(), atqa.len(), a_crc.as_mut_ptr()) };
        assert_eq!(a_crc, [0xca, 0x15]);

        let mut atqb = [0x05u8, 0x00, 0x08];
        let mut b_crc = [0u8; 2];
        unsafe { iso14443b_crc(atqb.as_mut_ptr(), atqb.len(), b_crc.as_mut_ptr()) };
        assert_eq!(b_crc, iso14443b_crc_bytes(&atqb));

        let mut appended = [0x26u8, 0x00, 0x00];
        unsafe { iso14443a_crc_append(appended.as_mut_ptr(), 1) };
        assert_eq!(appended[1..], a_crc);
    }

    #[test]
    fn locate_historical_bytes_matches_existing_ats_layout() {
        let mut ats = [0x75u8, 0x77, 0x81, 0x02, 0x80, 0x80];
        let mut tk_len = 0usize;
        let ptr = unsafe {
            iso14443a_locate_historical_bytes(
                ats.as_mut_ptr(),
                ats.len(),
                ptr::addr_of_mut!(tk_len),
            )
        };
        assert_eq!(tk_len, 2);
        assert_eq!(unsafe { slice::from_raw_parts(ptr, tk_len) }, [0x80, 0x80]);
    }
}
