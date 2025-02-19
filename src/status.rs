//! A Status Report announces the station's current mission or any other single
//! line status to everyone. The report starts with the '>' APRS Data Type Identifier.
//! The report may optionally contain a timestamp.
//!
//! Examples:
//! - ">12.6V 0.2A 22degC"              (report without timestamp)
//! - ">120503hFatal error"             (report with timestamp in HMS format)
//! - ">281205zSystem will shutdown"    (report with timestamp in DHM format)

use std::convert::TryFrom;
use std::io::Write;

use Callsign;
use DecodeError;
use DhmTimestamp;
use EncodeError;
use Timestamp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AprsStatus {
    pub to: Callsign,
    pub data_type_identifier: u8,

    timestamp: Option<Timestamp>,
    comment: Vec<u8>,
}

impl AprsStatus {
    pub fn new(to: Callsign, timestamp: Option<DhmTimestamp>, comment: Vec<u8>) -> Self {
        let timestamp = timestamp.map(|t| t.into());
        Self {
            to,
            data_type_identifier: b'>',
            timestamp,
            comment,
        }
    }

    /// According to APRS spec, an AprsStatus should only allow the DDHHMM timestamp. (See page 80 of APRS101.PDF)
    /// In practice, many encoders don't adhere to this.
    /// Use this function to create an AprsStatus with any timestamp type
    pub fn new_noncompliant(to: Callsign, timestamp: Option<Timestamp>, comment: Vec<u8>) -> Self {
        Self {
            data_type_identifier: b'>',
            to,
            timestamp,
            comment,
        }
    }

    pub fn is_timestamp_compliant(&self) -> bool {
        self.timestamp
            .as_ref()
            .map(|t| matches!(t, Timestamp::DDHHMM(_, _, _)))
            .unwrap_or(true)
    }

    pub fn timestamp(&self) -> Option<&Timestamp> {
        self.timestamp.as_ref()
    }

    pub fn comment(&self) -> &[u8] {
        &self.comment
    }

    pub fn decode(b: &[u8], to: Callsign) -> Result<Self, DecodeError> {
        // Interpret the first 7 bytes as a timestamp, if valid.
        // Otherwise the whole field is the comment.
        let timestamp = b.get(..7).and_then(|b| Timestamp::try_from(b).ok());
        let comment = if timestamp.is_some() { &b[7..] } else { b };

        Ok(AprsStatus {
            to,
            data_type_identifier: b'>',
            timestamp,
            comment: comment.to_owned(),
        })
    }

    pub fn encode<W: Write>(&self, buf: &mut W) -> Result<(), EncodeError> {
        write!(buf, ">")?;

        if let Some(ts) = &self.timestamp {
            ts.encode(buf)?;
        }

        buf.write_all(&self.comment)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_callsign() -> Callsign {
        Callsign::new_no_ssid("VE9")
    }

    #[test]
    fn parse_without_timestamp_or_comment() {
        let result = AprsStatus::decode(&b""[..], default_callsign()).unwrap();

        assert_eq!(result.to, default_callsign());
        assert_eq!(result.timestamp, None);
        assert_eq!(result.comment, []);
    }

    #[test]
    fn parse_with_timestamp_without_comment() {
        let result = AprsStatus::decode(r"312359z".as_bytes(), default_callsign()).unwrap();

        assert_eq!(result.to, default_callsign());
        assert_eq!(result.timestamp, Some(Timestamp::DDHHMM(31, 23, 59)));
        assert_eq!(result.comment, b"");
    }

    #[test]
    fn parse_without_timestamp_with_comment() {
        let result = AprsStatus::decode(&b"Hi there!"[..], default_callsign()).unwrap();

        assert_eq!(result.to, default_callsign());
        assert_eq!(result.timestamp, None);
        assert_eq!(result.comment, b"Hi there!");
    }

    #[test]
    fn parse_with_timestamp_and_comment() {
        let result =
            AprsStatus::decode(r"235959hHi there!".as_bytes(), default_callsign()).unwrap();

        assert_eq!(result.to, default_callsign());
        assert_eq!(result.timestamp, Some(Timestamp::HHMMSS(23, 59, 59)));
        assert_eq!(result.comment, b"Hi there!");
    }

    #[test]
    fn compliant_time_is_compliant() {
        let result = AprsStatus::decode(r"312359z".as_bytes(), default_callsign()).unwrap();

        assert_eq!(result.to, default_callsign());
        assert_eq!(result.timestamp, Some(Timestamp::DDHHMM(31, 23, 59)));
        assert!(result.is_timestamp_compliant());
    }

    #[test]
    fn uncompliant_time_is_not_compliant() {
        let result =
            AprsStatus::decode(r"235959hHi there!".as_bytes(), default_callsign()).unwrap();

        assert_eq!(result.to, default_callsign());
        assert_eq!(result.timestamp, Some(Timestamp::HHMMSS(23, 59, 59)));
        assert!(!result.is_timestamp_compliant());
    }

    #[test]
    fn missing_time_is_compliant() {
        let result = AprsStatus::decode(&b"Hi there!"[..], default_callsign()).unwrap();

        assert_eq!(result.to, default_callsign());
        assert_eq!(result.timestamp, None);
        assert!(result.is_timestamp_compliant());
    }
}
