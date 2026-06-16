use super::*;

pub(super) fn bits_to_bytes_len(bits_len: usize) -> usize {
    bits_len.div_ceil(8)
}

pub(super) fn raw_frame_bits_len(bytes_len: usize, last_bits: u8) -> usize {
    if bytes_len == 0 {
        0
    } else if last_bits == 0 {
        bytes_len * 8
    } else {
        (bytes_len.saturating_sub(1) * 8) + usize::from(last_bits)
    }
}

fn mirror(byte: u8) -> u8 {
    byte.reverse_bits()
}

pub(super) fn pn53x_wrap_frame(
    tx: &[u8],
    tx_bits_len: usize,
    tx_parity: Option<&[u8]>,
) -> Result<Vec<u8>, Error> {
    if tx_bits_len == 0 {
        return Ok(Vec::new());
    }
    let tx_bytes_len = bits_to_bytes_len(tx_bits_len);
    if tx.len() < tx_bytes_len {
        return Err(status_error("pn53x_wrap_frame", NFC_EINVARG));
    }
    if tx_bits_len < 9 {
        return Ok(vec![tx[0]]);
    }

    let parity = tx_parity.ok_or(Error::InvalidArgument("tx_parity"))?;
    let full_bytes = tx_bits_len / 8;
    if parity.len() < full_bytes {
        return Err(status_error("pn53x_wrap_frame", NFC_EINVARG));
    }

    let frame_bits_len = tx_bits_len + full_bytes;
    let frame_bytes_len = bits_to_bytes_len(frame_bits_len);
    let mut frame = vec![0u8; frame_bytes_len];
    let mut bits_left = tx_bits_len;
    let mut data_pos = 0usize;
    let mut frame_pos = 0usize;
    loop {
        let mut frame_byte = 0u8;
        for bit_pos in 0..8 {
            let data = mirror(tx[data_pos]);
            frame_byte |= data >> bit_pos;
            frame[frame_pos] = mirror(frame_byte);
            frame_byte = ((u16::from(data)) << (8 - bit_pos)) as u8;
            frame_byte |= (parity[data_pos] & 0x01) << (7 - bit_pos);
            frame_pos += 1;
            if frame_pos >= frame.len() {
                return Ok(frame);
            }
            frame[frame_pos] = mirror(frame_byte);
            data_pos += 1;
            if bits_left < 9 {
                return Ok(frame);
            }
            bits_left -= 8;
        }
        frame_pos += 1;
        if frame_pos >= frame.len() {
            return Ok(frame);
        }
    }
}

pub(super) fn pn53x_unwrap_frame(
    frame: &[u8],
    frame_bits_len: usize,
    rx: &mut [u8],
    mut rx_parity: Option<&mut [u8]>,
) -> Result<usize, Error> {
    if frame_bits_len == 0 {
        return Ok(0);
    }
    let frame_bytes_len = bits_to_bytes_len(frame_bits_len);
    if frame.len() < frame_bytes_len {
        return Err(status_error("pn53x_unwrap_frame", NFC_EIO));
    }
    if frame_bits_len < 9 {
        if rx.is_empty() {
            return Err(status_error("pn53x_unwrap_frame", NFC_EOVFLOW));
        }
        rx[0] = frame[0];
        return Ok(frame_bits_len);
    }

    let rx_bits_len = frame_bits_len - (frame_bits_len / 9);
    let rx_bytes_len = bits_to_bytes_len(rx_bits_len);
    if rx.len() < rx_bytes_len {
        return Err(status_error("pn53x_unwrap_frame", NFC_EOVFLOW));
    }
    if let Some(parity) = rx_parity.as_ref()
        && parity.len() < rx_bits_len / 8
    {
        return Err(status_error("pn53x_unwrap_frame", NFC_EOVFLOW));
    }

    let mut bits_left = frame_bits_len;
    let mut data_pos = 0usize;
    let mut frame_pos = 0usize;
    loop {
        for bit_pos in 0..8 {
            let first = mirror(frame[frame_pos + data_pos]);
            let second = mirror(frame[frame_pos + data_pos + 1]);
            let mut data = ((u16::from(first)) << bit_pos) as u8;
            data |= (u16::from(second) >> (8 - bit_pos)) as u8;
            rx[data_pos] = mirror(data);
            if let Some(parity) = rx_parity.as_deref_mut() {
                parity[data_pos] = (second >> (7 - bit_pos)) & 0x01;
            }
            data_pos += 1;
            if bits_left <= 9 {
                return Ok(rx_bits_len);
            }
            bits_left -= 9;
        }
        frame_pos += 1;
    }
}

pub(super) fn even_parity_bit(byte: u8) -> u8 {
    u8::from(byte.count_ones().is_multiple_of(2))
}

fn iso14443a_crc_append(data: &[u8]) -> [u8; 2] {
    let mut crc = 0x6363u16;
    for byte in data {
        let mut value = *byte ^ (crc as u8);
        value ^= value << 4;
        crc = (crc >> 8)
            ^ (u16::from(value) << 8)
            ^ (u16::from(value) << 3)
            ^ (u16::from(value) >> 4);
    }
    [crc as u8, (crc >> 8) as u8]
}

fn iso14443b_crc_append(data: &[u8]) -> [u8; 2] {
    let mut crc = 0xFFFFu16;
    for byte in data {
        let mut value = *byte ^ (crc as u8);
        value ^= value << 4;
        crc = (crc >> 8)
            ^ (u16::from(value) << 8)
            ^ (u16::from(value) << 3)
            ^ (u16::from(value) >> 4);
    }
    crc = !crc;
    [crc as u8, (crc >> 8) as u8]
}

pub(super) fn timer_last_command_byte(tx: &[u8], txmode: Option<u8>) -> Result<u8, Error> {
    let Some(&last) = tx.last() else {
        return Err(status_error("pn53x_timer_last_byte", NFC_EINVARG));
    };
    let Some(txmode) = txmode else {
        return Ok(last);
    };
    if txmode & SYMBOL_TX_CRC_ENABLE == 0 {
        return Ok(last);
    }
    let crc = match txmode & SYMBOL_TX_FRAMING {
        0x00 => iso14443a_crc_append(tx),
        0x03 => iso14443b_crc_append(tx),
        _ => return Ok(last),
    };
    Ok(crc[1])
}
