/*!
 * edf-reader parse metadata of EDF file and can read block of data from this EDF file
 * spec of EDF format : https://www.edfplus.info/specs/edf.html
 *
 */

extern crate chrono;
extern crate futures;

#[macro_use]
extern crate serde_derive;

pub mod async_reader;
pub mod file_reader;
pub mod model;
mod parser;
pub mod sync_reader;

use std::convert::TryInto;

use model::EDFHeader;

use std::io::{Error, ErrorKind};

fn get_sample(data: &Vec<u8>, index: usize) -> Result<i16, std::io::Error> {
    let start = 2 * index;
    let end = start + 2;

    // Ensure the indices are within the bounds of the data vector
    if end > data.len() {
        return Err(std::io::Error::new(
             std::io::ErrorKind::UnexpectedEof,
             format!("Attempted to read sample bytes beyond data vector bounds (index: {}, needed: {}, len: {})", index, end, data.len())
         ));
    }

    // Get the 2-byte slice corresponding to the sample
    let sample_bytes_slice = &data[start..end];

    // Try to convert the slice into a fixed-size array [u8; 2]
    let sample_bytes_array: [u8; 2] = sample_bytes_slice.try_into().map_err(|e| {
        // This error should theoretically not happen if bounds check passed and length is 2
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to convert byte slice to [u8; 2]: {}", e),
        )
    })?;

    // Construct the i16 using the standard library function for little-endian bytes
    Ok(i16::from_le_bytes(sample_bytes_array))
}

fn check_bounds(start_time: u64, duration: u64, edf_header: &EDFHeader) -> Result<(), Error> {
    if start_time + duration > edf_header.block_duration * edf_header.number_of_blocks {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Window is out of bounds",
        ));
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::get_sample;
    use std::io::ErrorKind;

    // Tests successful conversion of little-endian byte pairs to i16.
    #[test]
    fn test_get_sample_le_conversion_success() {
        // Test case 1: Positive value
        // Bytes [200, 1] in little-endian correspond to 0x01C8 = 456
        let data1 = vec![200, 1];
        let result1 = get_sample(&data1, 0);
        assert!(
            result1.is_ok(),
            "Expected Ok for data1, got Err: {:?}",
            result1.err()
        );
        assert_eq!(456, result1.unwrap());

        // Test case 2: Negative value
        // Bytes [44, 238] in little-endian correspond to 0xEE2C = -4564 (signed 16-bit)
        let data2 = vec![44, 238];
        let result2 = get_sample(&data2, 0);
        assert!(
            result2.is_ok(),
            "Expected Ok for data2, got Err: {:?}",
            result2.err()
        );
        assert_eq!(-4564, result2.unwrap());

        // Test case 3: Multiple samples in one vector
        let data3 = vec![200, 1, 44, 238];
        let result3a = get_sample(&data3, 0);
        assert!(
            result3a.is_ok(),
            "Expected Ok for data3[0], got Err: {:?}",
            result3a.err()
        );
        assert_eq!(456, result3a.unwrap());

        let result3b = get_sample(&data3, 1);
        assert!(
            result3b.is_ok(),
            "Expected Ok for data3[1], got Err: {:?}",
            result3b.err()
        );
        assert_eq!(-4564, result3b.unwrap());
    }

    // Tests that attempting to read beyond the bounds of the data vector returns an error.
    #[test]
    fn test_get_sample_out_of_bounds() {
        // Test case 1: Index requires bytes beyond vector length
        let data1 = vec![200, 1];
        let result1 = get_sample(&data1, 1);
        assert!(
            result1.is_err(),
            "Expected Err when reading index 1 from data of length 2"
        );
        // Check the specific error kind
        match result1 {
            Err(e) => assert_eq!(
                e.kind(),
                ErrorKind::UnexpectedEof,
                "Expected UnexpectedEof error kind"
            ),
            Ok(_) => panic!("Expected error but got Ok"),
        }

        // Test case 2: Index is valid, but requires second byte which is out of bounds
        let data2 = vec![200]; // Length 1. Index 0 needs bytes 0, 1.
        let result2 = get_sample(&data2, 0);
        assert!(
            result2.is_err(),
            "Expected Err when reading index 0 from data of length 1"
        );
        match result2 {
            Err(e) => assert_eq!(
                e.kind(),
                ErrorKind::UnexpectedEof,
                "Expected UnexpectedEof error kind"
            ),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    /// Tests that calling get_sample on an empty data vector returns an error.
    #[test]
    fn test_get_sample_empty_data() {
        let data: Vec<u8> = vec![];
        let result = get_sample(&data, 0); // Attempt to read sample 0 from empty vec
        assert!(result.is_err(), "Expected Err when reading from empty data");
        match result {
            Err(e) => assert_eq!(
                e.kind(),
                ErrorKind::UnexpectedEof,
                "Expected UnexpectedEof error kind"
            ),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }
}
