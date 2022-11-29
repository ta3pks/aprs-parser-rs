use std::io::Write;
use std::ops::Deref;

use base91;
use bytes::parse_bytes;
use AprsError;
use EncodeError;
use Precision;

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Default)]
pub struct Latitude(f64);

impl Deref for Latitude {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Latitude {
    /// Creates a new `Latitude`.
    /// Returns `None` if the given value is not a valid latitude.
    pub fn new(value: f64) -> Option<Self> {
        if value > 90.0 || value < -90.0 || value.is_nan() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The value of the latitude.
    pub fn value(&self) -> f64 {
        self.0
    }

    pub(crate) fn parse_uncompressed(b: &[u8]) -> Result<(Self, Precision), AprsError> {
        if b.len() != 8 || b[4] != b'.' {
            return Err(AprsError::InvalidLatitude(b.to_owned()));
        }

        let north = match b[7] {
            b'N' => true,
            b'S' => false,
            _ => return Err(AprsError::InvalidLatitude(b.to_owned())),
        };

        // Some APRS lats have trailing spaces
        // This is used to convey ambiguity and is only valid in latitudes
        // Once we encounter a space, the remainder must be spaces
        let mut total_spaces = 0;
        let (deg, num_spaces) = parse_bytes_trailing_spaces(&[b[0], b[1]], false)
            .ok_or_else(|| AprsError::InvalidLatitude(b.to_owned()))?;
        total_spaces += num_spaces;
        let (min, num_spaces) = parse_bytes_trailing_spaces(&[b[2], b[3]], num_spaces > 0)
            .ok_or_else(|| AprsError::InvalidLatitude(b.to_owned()))?;
        total_spaces += num_spaces;
        let (min_frac, num_spaces) = parse_bytes_trailing_spaces(&[b[5], b[6]], num_spaces > 0)
            .ok_or_else(|| AprsError::InvalidLatitude(b.to_owned()))?;
        total_spaces += num_spaces;

        let precision = Precision::from_num_digits(total_spaces)
            .ok_or_else(|| AprsError::InvalidLatitude(b.to_owned()))?;

        let value = deg as f64 + min as f64 / 60. + min_frac as f64 / 6_000.;
        let value = if north { value } else { -value };

        let lat = Self::new(value).ok_or_else(|| AprsError::InvalidLatitude(b.to_owned()))?;

        Ok((lat, precision))
    }

    pub(crate) fn parse_compressed(b: &[u8]) -> Result<Self, AprsError> {
        let value = 90.0
            - (base91::decode_ascii(b).ok_or_else(|| AprsError::InvalidLatitude(b.to_owned()))?
                / 380926.0);

        Self::new(value).ok_or_else(|| AprsError::InvalidLatitude(b.to_owned()))
    }

    pub(crate) fn encode_compressed<W: Write>(&self, buf: &mut W) -> Result<(), EncodeError> {
        let value = (90.0 - self.0) * 380926.0;
        base91::encode_ascii(value, buf, 4)
    }

    pub(crate) fn encode_uncompressed<W: Write>(
        &self,
        buf: &mut W,
        precision: Precision,
    ) -> Result<(), EncodeError> {
        let lat = self.0;

        let (dir, lat) = if lat >= 0.0 { ('N', lat) } else { ('S', -lat) };

        let deg = lat as u32;
        let min = ((lat - (deg as f64)) * 60.0) as u32;
        let min_frac = ((lat - (deg as f64) - (min as f64 / 60.0)) * 6000.0).round() as u32;

        // zero out fields as required for precision
        // Ideally we would be doing some clever rounding here
        // E.g. if last 2 digits were blanked,
        // 4905.83 would become 4906.__
        let mut digit_buffer = [b' '; 6];
        let blank_index = 6 - precision.num_digits() as usize;

        // write will only fail if there isn't enough space
        // which is what we want (the remaining buffer should remain untouched)
        let _ = write!(
            &mut digit_buffer[..blank_index],
            "{:02}{:02}{:02}",
            deg,
            min,
            min_frac
        );
        buf.write_all(&digit_buffer[0..4])?;
        write!(buf, ".")?;
        buf.write_all(&digit_buffer[4..6])?;
        write!(buf, "{}", dir)?;
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Default)]
pub struct Longitude(f64);

impl Deref for Longitude {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Longitude {
    /// Creates a new `Longitude`.
    /// Returns `None` if the given value is not a valid longitude
    pub fn new(value: f64) -> Option<Self> {
        if value > 180.0 || value < -180.0 || value.is_nan() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The value of the longitude.
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Precision is needed so we know how many digits to ignore
    pub(crate) fn parse_uncompressed(b: &[u8], precision: Precision) -> Result<Self, AprsError> {
        if b.len() != 9 || b[5] != b'.' {
            return Err(AprsError::InvalidLongitude(b.to_owned()));
        }

        let east = match b[8] {
            b'E' => true,
            b'W' => false,
            _ => return Err(AprsError::InvalidLongitude(b.to_owned())),
        };

        let mut digit_buffer = [0; 7];
        digit_buffer[0..5].copy_from_slice(&b[0..5]);
        digit_buffer[5..7].copy_from_slice(&b[6..8]);

        // zero out the digits we don't care about
        for i in (7 - precision.num_digits())..7 {
            digit_buffer[i as usize] = b'0';
        }

        let deg = parse_bytes::<u32>(&digit_buffer[0..3])
            .ok_or_else(|| AprsError::InvalidLongitude(b.to_owned()))? as f64;
        let min = parse_bytes::<u32>(&digit_buffer[3..5])
            .ok_or_else(|| AprsError::InvalidLongitude(b.to_owned()))? as f64;
        let min_frac = parse_bytes::<u32>(&digit_buffer[5..7])
            .ok_or_else(|| AprsError::InvalidLongitude(b.to_owned()))?
            as f64;

        let value = deg + min / 60. + min_frac / 6_000.;
        let value = if east { value } else { -value };

        Self::new(value).ok_or_else(|| AprsError::InvalidLongitude(b.to_owned()))
    }

    pub(crate) fn parse_compressed(b: &[u8]) -> Result<Self, AprsError> {
        let value = (base91::decode_ascii(b)
            .ok_or_else(|| AprsError::InvalidLongitude(b.to_owned()))?
            / 190463.0)
            - 180.0;

        Self::new(value).ok_or_else(|| AprsError::InvalidLongitude(b.to_owned()))
    }

    pub(crate) fn encode_compressed<W: Write>(&self, buf: &mut W) -> Result<(), EncodeError> {
        let value = (180.0 + self.0) * 190463.0;
        base91::encode_ascii(value, buf, 4)
    }

    pub(crate) fn encode_uncompressed<W: Write>(&self, buf: &mut W) -> Result<(), EncodeError> {
        let lon = self.0;

        let (dir, lon) = if lon >= 0.0 { ('E', lon) } else { ('W', -lon) };

        let deg = lon as u32;
        let min = ((lon - (deg as f64)) * 60.0) as u32;
        let min_frac = ((lon - (deg as f64) - (min as f64 / 60.0)) * 6000.0).round() as u32;

        write!(buf, "{:03}{:02}.{:02}{}", deg, min, min_frac, dir)?;
        Ok(())
    }
}

// if only_spaces is true, requires that b is only spaces
// returns the parsed value as well as the number of spaces we found
fn parse_bytes_trailing_spaces(b: &[u8; 2], only_spaces: bool) -> Option<(u32, u8)> {
    if only_spaces {
        if b == &[b' ', b' '] {
            return Some((0, 2));
        } else {
            return None;
        }
    }
    match (b[0], b[1]) {
        (b' ', b' ') => Some((0, 2)),
        (_, b' ') => parse_bytes::<u32>(&b[0..1]).map(|v| (v * 10, 1)),
        (_, _) => parse_bytes::<u32>(&b[..]).map(|v| (v, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latitude_out_of_bounds() {
        assert_eq!(None, Latitude::new(90.1));
        assert_eq!(None, Latitude::new(-90.1));
    }

    #[test]
    fn test_longitude_out_of_bounds() {
        assert_eq!(None, Latitude::new(180.1));
        assert_eq!(None, Latitude::new(-180.1));
    }

    #[test]
    fn test_parse_bytes_trailing_spaces() {
        assert_eq!(Some((12, 0)), parse_bytes_trailing_spaces(b"12", false));
        assert_eq!(Some((10, 1)), parse_bytes_trailing_spaces(b"1 ", false));
        assert_eq!(Some((0, 2)), parse_bytes_trailing_spaces(b"  ", false));

        assert_eq!(None, parse_bytes_trailing_spaces(b" 2", false));

        assert_eq!(None, parse_bytes_trailing_spaces(b"12", true));
        assert_eq!(None, parse_bytes_trailing_spaces(b"1 ", true));
        assert_eq!(None, parse_bytes_trailing_spaces(b" 1", true));
        assert_eq!(Some((0, 2)), parse_bytes_trailing_spaces(b"  ", true));
    }

    #[test]
    fn test_parse_uncompressed_latitude() {
        assert_eq!(
            Latitude::parse_uncompressed(&b"4903.50N"[..]).unwrap(),
            (
                Latitude::new(49.05833333333333).unwrap(),
                Precision::HundredthMinute
            )
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"4903.50S"[..]).unwrap(),
            (
                Latitude::new(-49.05833333333333).unwrap(),
                Precision::HundredthMinute
            )
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"4903.5 S"[..]).unwrap(),
            (
                Latitude::new(-49.05833333333333).unwrap(),
                Precision::TenthMinute
            )
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"4903.  S"[..]).unwrap(),
            (Latitude::new(-49.05).unwrap(), Precision::OneMinute)
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"490 .  S"[..]).unwrap(),
            (Latitude::new(-49.0).unwrap(), Precision::TenMinute)
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"4   .  S"[..]).unwrap(),
            (Latitude::new(-40.0).unwrap(), Precision::TenDegree)
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"    .  S"[..]),
            Err(AprsError::InvalidLatitude(b"    .  S".to_vec()))
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"49 3.50W"[..]),
            Err(AprsError::InvalidLatitude(b"49 3.50W".to_vec()))
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"490 .50W"[..]),
            Err(AprsError::InvalidLatitude(b"490 .50W".to_vec()))
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"49  . 0W"[..]),
            Err(AprsError::InvalidLatitude(b"49  . 0W".to_vec()))
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"4903.50W"[..]),
            Err(AprsError::InvalidLatitude(b"4903.50W".to_vec()))
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"4903.50E"[..]),
            Err(AprsError::InvalidLatitude(b"4903.50E".to_vec()))
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"9903.50N"[..]),
            Err(AprsError::InvalidLatitude(b"9903.50N".to_vec()))
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"0000.00N"[..]).unwrap(),
            (Latitude::new(0.0).unwrap(), Precision::HundredthMinute)
        );
        assert_eq!(
            Latitude::parse_uncompressed(&b"0000.00S"[..]).unwrap(),
            (Latitude::new(0.0).unwrap(), Precision::HundredthMinute)
        );
    }

    #[test]
    fn test_parse_uncompressed_longitude() {
        assert_relative_eq!(
            *Longitude::parse_uncompressed(&b"12903.50E"[..], Precision::default()).unwrap(),
            129.05833333333333
        );
        assert_relative_eq!(
            *Longitude::parse_uncompressed(&b"04903.50W"[..], Precision::default()).unwrap(),
            -49.05833333333333
        );
        assert_eq!(
            Longitude::parse_uncompressed(&b"04903.50N"[..], Precision::default()),
            Err(AprsError::InvalidLongitude(b"04903.50N".to_vec()))
        );
        assert_eq!(
            Longitude::parse_uncompressed(&b"04903.50S"[..], Precision::default()),
            Err(AprsError::InvalidLongitude(b"04903.50S".to_vec()))
        );
        assert_eq!(
            Longitude::parse_uncompressed(&b"18903.50E"[..], Precision::default()),
            Err(AprsError::InvalidLongitude(b"18903.50E".to_vec()))
        );
        assert_relative_eq!(
            *Longitude::parse_uncompressed(&b"00000.00E"[..], Precision::default()).unwrap(),
            0.0
        );
        assert_relative_eq!(
            *Longitude::parse_uncompressed(&b"00000.00W"[..], Precision::default()).unwrap(),
            0.0
        );
        assert_relative_eq!(
            *Longitude::parse_uncompressed(&b"00000.ZZW"[..], Precision::OneMinute).unwrap(),
            0.0
        );
        assert_relative_eq!(
            *Longitude::parse_uncompressed(&b"00000.98W"[..], Precision::OneMinute).unwrap(),
            0.0
        );
    }

    #[test]
    fn test_encode_uncompressed_latitude() {
        let mut buf = vec![];
        Latitude::new(49.05833)
            .unwrap()
            .encode_uncompressed(&mut buf, Precision::default())
            .unwrap();
        assert_eq!(buf, &b"4903.50N"[..]);

        let mut buf = vec![];
        Latitude::new(-49.05833)
            .unwrap()
            .encode_uncompressed(&mut buf, Precision::default())
            .unwrap();
        assert_eq!(buf, &b"4903.50S"[..]);

        let mut buf = vec![];
        Latitude::new(0.0)
            .unwrap()
            .encode_uncompressed(&mut buf, Precision::default())
            .unwrap();
        assert_eq!(buf, &b"0000.00N"[..]);

        let mut buf = vec![];
        Latitude::new(-49.05833)
            .unwrap()
            .encode_uncompressed(&mut buf, Precision::OneMinute)
            .unwrap();
        assert_eq!(buf, &b"4903.  S"[..]);
    }

    #[test]
    fn test_encode_uncompressed_longitude() {
        let mut buf = vec![];
        Longitude::new(129.05833)
            .unwrap()
            .encode_uncompressed(&mut buf)
            .unwrap();
        assert_eq!(buf, &b"12903.50E"[..]);

        let mut buf = vec![];
        Longitude::new(-49.0583)
            .unwrap()
            .encode_uncompressed(&mut buf)
            .unwrap();
        assert_eq!(buf, &b"04903.50W"[..]);

        let mut buf = vec![];
        Longitude(0.0).encode_uncompressed(&mut buf).unwrap();
        assert_eq!(buf, &b"00000.00E"[..]);
    }
}
