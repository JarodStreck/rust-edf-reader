//! Read an EDF file asynhronously (with futures)

use crate::file_reader::AsyncFileReader;
use crate::model::{EDFHeader, EDF_HEADER_BYTE_SIZE};

use futures::future::{err, ok, Future};
use std::io::Error;

pub struct AsyncEDFReader<T: AsyncFileReader> {
    pub edf_header: EDFHeader,
    file_reader: T,
}

impl<T: 'static + AsyncFileReader + Send + Sync + Clone> AsyncEDFReader<T> {
    /**
    Init an EDFReader with a custom FileReader.
    It can be usefull if the EDF file is not located in the system file. (ie : we cannot use RandomAccessFile).
    An example of use : read the file with DOM FileAPI in Webassembly
    */
    pub fn init_with_file_reader(
        file_reader: T,
    ) -> Box<dyn Future<Item = AsyncEDFReader<T>, Error = std::io::Error> + Send> {
        let reader_clone_for_channels = file_reader;
        let reader_final = reader_clone_for_channels.clone();
        Box::new(reader_clone_for_channels.read_async(0, 256).and_then(
            move |general_header_raw: Vec<u8>| {
                let mut edf_header = EDFHeader::build_general_header(general_header_raw);
                let channel_header_len = edf_header.number_of_signals * EDF_HEADER_BYTE_SIZE as u64;

                reader_final.read_async(256, channel_header_len).and_then(
                    move |channel_headers_raw| {
                        edf_header.build_channel_headers(channel_headers_raw);
                        ok(AsyncEDFReader {
                            edf_header: edf_header,
                            file_reader: reader_final,
                        })
                    },
                )
            },
        ))
    }

    /// Reads a window of EDF data asynchronously.
    pub fn read_data_window(
        &self,
        start_time_ms: u64,
        duration_ms: u64,
    ) -> Box<dyn Future<Item = Vec<Vec<f32>>, Error = std::io::Error> + Send> {
        if let Err(e) = super::check_bounds(start_time_ms, duration_ms, &self.edf_header) {
            return Box::new(err::<Vec<Vec<f32>>, Error>(e));
        }
        // calculate the corresponding blocks to get

        let first_block_start_time = start_time_ms - start_time_ms % self.edf_header.block_duration;
        let first_block_index = first_block_start_time / self.edf_header.block_duration;
        let number_of_blocks_to_get =
            (duration_ms as f64 / self.edf_header.block_duration as f64).ceil() as u64;
        let offset = self.edf_header.byte_size_header
            + first_block_index * self.edf_header.get_size_of_data_block();
        let length_to_read = number_of_blocks_to_get * self.edf_header.get_size_of_data_block();

        let header = self.edf_header.clone();

        let processing_future = self
            .file_reader
            .read_async(offset, length_to_read)
            .and_then(move |data: Vec<u8>| -> Result<Vec<Vec<f32>>, Error> {
                let mut result: Vec<Vec<f32>> =
                    Vec::with_capacity(header.number_of_signals as usize);
                for _ in 0..header.number_of_signals {
                    result.push(Vec::new());
                }

                let mut index = 0;

                for _block_idx in 0..number_of_blocks_to_get {
                    for (j, channel) in header.channels.iter().enumerate() {
                        for _sample_idx in 0..channel.number_of_samples_in_data_record {
                            let digital_sample = match super::get_sample(&data, index) {
                                Ok(s) => s as f32,
                                Err(e) => {
                                    eprintln!(
                                        "Error reading digital sample at byte index {}: {}",
                                        index * 2,
                                        e
                                    );
                                    return Err(e);
                                }
                            };
                            result[j].push(
                                (digital_sample - channel.digital_minimum as f32)
                                    * channel.scale_factor
                                    + channel.physical_minimum,
                            );
                            index += 1;
                        }
                    }
                }

                Ok(result)
            });

        Box::new(processing_future)
    }
}
