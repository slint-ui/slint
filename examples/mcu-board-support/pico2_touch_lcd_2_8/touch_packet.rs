// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub fn point(data: &[u8; 6]) -> Option<(u16, u16)> {
    if !(1..=5).contains(&(data[5] & 15)) || data[0] & 15 != 6 {
        return None;
    }
    let x = (u16::from(data[1]) << 4) | u16::from(data[3] >> 4);
    let y = (u16::from(data[2]) << 4) | u16::from(data[3] & 15);
    if x < 240 && y < 320 { Some((y, 239 - x)) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_contact_uses_native_landscape_coordinates() {
        assert_eq!(point(&[6, 0, 0, 0, 0, 1]), Some((0, 239)));
        assert_eq!(point(&[6, 14, 19, 255, 0, 1]), Some((319, 0)));
        assert_eq!(point(&[6, 14, 19, 238, 0, 5]), Some((318, 1)));
        assert_eq!(point(&[6, 14, 19, 255, 0, 0]), None);
        assert_eq!(point(&[4, 14, 19, 255, 0, 1]), None);
        assert_eq!(point(&[6, 14, 19, 255, 0, 6]), None);
        assert_eq!(point(&[6, 15, 19, 255, 0, 1]), None);
        assert_eq!(point(&[6, 14, 20, 255, 0, 1]), None);
    }
}
