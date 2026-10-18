// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub const REPORT_BYTES: usize = 27;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Report {
    Contact(u16, u16),
    Released,
    Invalid,
}

#[derive(Clone, Copy)]
struct Contact {
    id: u8,
    x: u16,
    y: u16,
}

#[derive(Default)]
pub struct Tracker {
    previous: Option<Contact>,
}

impl Tracker {
    pub fn read(&mut self, data: &[u8; REPORT_BYTES]) -> Report {
        let count = data[5] & 15;
        if data[6] != 0xab || count > 5 || data[5] & 0x80 != 0 {
            return Report::Invalid;
        }
        // A cleared count with an unchanged active record is an acknowledged report, not a lift.
        if count == 0 && (data[0] & 15 == 6 || data[0] == 0xab) {
            return Report::Invalid;
        }
        let mut selected: Option<Contact> = None;
        let mut invalid = false;
        for index in 0..usize::from(count) {
            let offset = if index == 0 { 0 } else { 7 + (index - 1) * 5 };
            let point = &data[offset..offset + 5];
            match point[0] & 15 {
                0 => continue,
                6 => {}
                _ => {
                    invalid = true;
                    continue;
                }
            }
            let x = (u16::from(point[1]) << 4) | u16::from(point[3] >> 4);
            let y = (u16::from(point[2]) << 4) | u16::from(point[3] & 15);
            if x >= 240 || y >= 320 {
                invalid = true;
                continue;
            }
            let contact = Contact { id: point[0] >> 4, x: y, y: 239 - x };
            let score = |candidate: Contact| -> u32 {
                self.previous.map_or(0, |previous| {
                    if candidate.id == previous.id {
                        0
                    } else {
                        1 + i32::from(candidate.x).abs_diff(i32::from(previous.x)).pow(2)
                            + i32::from(candidate.y).abs_diff(i32::from(previous.y)).pow(2)
                    }
                })
            };
            if selected.is_none_or(|old| score(contact) < score(old)) {
                selected = Some(contact);
            }
        }
        if let Some(contact) = selected {
            self.previous = Some(contact);
            Report::Contact(contact.x, contact.y)
        } else if invalid {
            Report::Invalid
        } else {
            self.previous = None;
            Report::Released
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(points: &[[u8; 5]]) -> [u8; REPORT_BYTES] {
        let mut data = [0; REPORT_BYTES];
        data[5] = points.len() as u8;
        data[6] = 0xab;
        for (index, point) in points.iter().enumerate() {
            let offset = if index == 0 { 0 } else { 7 + (index - 1) * 5 };
            data[offset..offset + 5].copy_from_slice(point);
        }
        data
    }

    #[test]
    fn native_landscape_coordinates_and_release() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.read(&report(&[[6, 0, 0, 0, 0]])), Report::Contact(0, 239));
        assert_eq!(tracker.read(&report(&[[6, 14, 19, 255, 0]])), Report::Contact(319, 0));
        assert_eq!(tracker.read(&report(&[[0, 14, 19, 255, 0]])), Report::Released);
    }

    #[test]
    fn lifted_first_contact_hands_over_without_releasing() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.read(&report(&[[6, 11, 11, 0xad, 0x47]])), Report::Contact(189, 53));
        let packet = report(&[[0, 11, 11, 0xad, 0x47], [0x16, 8, 11, 0x46, 0x45]]);
        assert_eq!(tracker.read(&packet), Report::Contact(182, 107));
        assert_eq!(tracker.read(&report(&[[0x16, 8, 11, 0x46, 0x45]])), Report::Contact(182, 107));
        assert_eq!(tracker.read(&report(&[[0x10, 8, 11, 0x46, 0x45]])), Report::Released);
    }

    #[test]
    fn contact_id_is_stable_when_record_order_changes() {
        let mut tracker = Tracker::default();
        let a = [0x16, 8, 11, 0x46, 0x45];
        let b = [6, 1, 2, 3, 4];
        assert_eq!(tracker.read(&report(&[a])), Report::Contact(182, 107));
        assert_eq!(tracker.read(&report(&[b, a])), Report::Contact(182, 107));
    }

    #[test]
    fn invalid_and_acknowledged_packets_do_not_release_a_held_contact() {
        let mut tracker = Tracker::default();
        let valid = report(&[[6, 8, 11, 0x46, 0x45]]);
        assert_eq!(tracker.read(&valid), Report::Contact(182, 107));
        let mut stale = valid;
        stale[5] = 0;
        assert_eq!(tracker.read(&stale), Report::Invalid);
        stale[0] = 0xab;
        assert_eq!(tracker.read(&stale), Report::Invalid);
        let mut bad = valid;
        bad[6] = 0;
        assert_eq!(tracker.read(&bad), Report::Invalid);
        bad = valid;
        bad[1] = 15;
        assert_eq!(tracker.read(&bad), Report::Invalid);
        bad = valid;
        bad[5] = 6;
        assert_eq!(tracker.read(&bad), Report::Invalid);
        assert_eq!(tracker.read(&valid), Report::Contact(182, 107));
        assert_eq!(tracker.read(&report(&[])), Report::Released);
    }
}
