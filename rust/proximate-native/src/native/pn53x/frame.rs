use super::*;

fn command_uses_status_byte(command: u8) -> bool {
    matches!(
        command,
        PN53X_READ_REGISTER
            | PN53X_IN_DATA_EXCHANGE
            | PN53X_IN_COMMUNICATE_THRU
            | PN53X_IN_JUMP_FOR_DEP
            | PN53X_TG_GET_DATA
            | PN53X_TG_SET_DATA
            | PN53X_TG_GET_INITIATOR_COMMAND
            | PN53X_TG_RESPONSE_TO_INITIATOR
            | PN53X_IN_DESELECT
    )
}

pub(super) fn split_status_response(command: u8, response: &[u8]) -> Result<(u8, Vec<u8>), Error> {
    if !command_uses_status_byte(command) {
        return Ok((0, response.to_vec()));
    }
    let Some((&status_flags, data)) = response.split_first() else {
        return Err(status_error("pn53x_status_response", NFC_EIO));
    };
    if status_flags & 0x80 != 0 {
        return Ok((PN53X_STATUS_NAD, data.to_vec()));
    }
    Ok((status_flags & 0x3f, data.to_vec()))
}

pub(crate) fn build_frame(payload: &[u8]) -> Result<Vec<u8>, Error> {
    if payload.is_empty() {
        return Err(Error::InvalidArgument("payload"));
    }

    if payload.len() > PN53X_EXTENDED_FRAME_DATA_MAX_LEN {
        return Err(Error::BufferTooSmall {
            needed: payload.len(),
            available: PN53X_EXTENDED_FRAME_DATA_MAX_LEN,
        });
    }

    let mut frame = Vec::with_capacity(PN532_BUFFER_LEN);
    if payload.len() <= 254 {
        let len = payload.len() as u8 + 1;
        frame.extend_from_slice(&[
            0x00,
            0x00,
            0xff,
            len,
            (!len).wrapping_add(1),
            HOST_TO_PN53X_TFI,
        ]);
        frame.extend_from_slice(payload);
    } else {
        let high = ((payload.len() + 1) >> 8) as u8;
        let low = ((payload.len() + 1) & 0xff) as u8;
        frame.extend_from_slice(&[
            0x00,
            0x00,
            0xff,
            0xff,
            0xff,
            high,
            low,
            (0u8).wrapping_sub(high.wrapping_add(low)),
            HOST_TO_PN53X_TFI,
        ]);
        frame.extend_from_slice(payload);
    }

    let dcs = payload
        .iter()
        .fold(0u8.wrapping_sub(HOST_TO_PN53X_TFI), |acc, byte| {
            acc.wrapping_sub(*byte)
        });
    frame.push(dcs);
    frame.push(0x00);
    Ok(frame)
}

pub(crate) fn is_ack_frame(frame: &[u8]) -> bool {
    frame.starts_with(&PN53X_ACK_FRAME)
}

pub(crate) fn build_response_frame(command: u8, payload: &[u8]) -> Result<Vec<u8>, Error> {
    if payload.len() > PN53X_EXTENDED_FRAME_DATA_MAX_LEN {
        return Err(Error::BufferTooSmall {
            needed: payload.len(),
            available: PN53X_EXTENDED_FRAME_DATA_MAX_LEN,
        });
    }

    let mut body = Vec::with_capacity(payload.len() + 2);
    body.push(PN53X_TO_HOST_TFI);
    body.push(command.wrapping_add(1));
    body.extend_from_slice(payload);

    let mut frame = Vec::with_capacity(body.len() + PN53X_EXTENDED_FRAME_OVERHEAD);
    if body.len() <= 0xfe {
        let len = body.len() as u8;
        frame.extend_from_slice(&[0x00, 0x00, 0xff, len, (!len).wrapping_add(1)]);
    } else {
        let high = (body.len() >> 8) as u8;
        let low = (body.len() & 0xff) as u8;
        frame.extend_from_slice(&[
            0x00,
            0x00,
            0xff,
            0xff,
            0xff,
            high,
            low,
            (0u8).wrapping_sub(high.wrapping_add(low)),
        ]);
    }
    frame.extend_from_slice(&body);
    let dcs = body
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    frame.push(dcs);
    frame.push(0x00);
    Ok(frame)
}

pub(crate) fn command_from_host_frame(frame: &[u8]) -> Result<u8, Error> {
    Ok(payload_from_host_frame(frame)?[0])
}

pub(crate) fn payload_from_host_frame(frame: &[u8]) -> Result<Vec<u8>, Error> {
    if frame.len() < 8 || !frame.starts_with(&[0x00, 0x00, 0xff]) {
        return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
    }

    let (body_offset, body_len) = if frame[3] == 0xff && frame[4] == 0xff {
        if frame.len() < 10 {
            return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
        }
        let length = ((frame[5] as usize) << 8) | frame[6] as usize;
        if frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]) != 0 {
            return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
        }
        (8, length)
    } else {
        if frame[3].wrapping_add(frame[4]) != 0 {
            return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
        }
        (5, frame[3] as usize)
    };

    let trailer_offset = body_offset + body_len;
    if frame.len() < trailer_offset + 2 || body_len < 2 || frame[body_offset] != HOST_TO_PN53X_TFI {
        return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
    }

    let body = &frame[body_offset..trailer_offset];
    let expected_dcs = body
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    if frame[trailer_offset] != expected_dcs || frame[trailer_offset + 1] != 0x00 {
        return Err(status_error("pn53x_command_from_host_frame", NFC_EIO));
    }

    Ok(body[1..].to_vec())
}

pub(super) fn parse_response_frame(frame: &[u8], expected_command: u8) -> Result<Vec<u8>, Error> {
    if frame.len() < 8 {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }
    if is_ack_frame(frame) {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }
    if !frame.starts_with(&[0x00, 0x00, 0xff]) {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    let (body_offset, body_len) = if frame[3] == 0xff && frame[4] == 0xff {
        if frame.len() < 11 {
            return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
        }
        let length = ((frame[5] as usize) << 8) | frame[6] as usize;
        let checksum = frame[5].wrapping_add(frame[6]).wrapping_add(frame[7]);
        if checksum != 0 {
            return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
        }
        (8usize, length)
    } else {
        let length = frame[3] as usize;
        let checksum = frame[3].wrapping_add(frame[4]);
        if checksum != 0 {
            return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
        }
        (5usize, length)
    };

    let trailer_offset = body_offset + body_len;
    if frame.len() < trailer_offset + 2 || body_len < 2 {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    let body = &frame[body_offset..trailer_offset];
    if body[0] != PN53X_TO_HOST_TFI || body[1] != expected_command.wrapping_add(1) {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    let expected_dcs = body
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg();
    if frame[trailer_offset] != expected_dcs || frame[trailer_offset + 1] != 0x00 {
        return Err(status_error("pn53x_parse_response_frame", NFC_EIO));
    }

    Ok(body[2..].to_vec())
}
