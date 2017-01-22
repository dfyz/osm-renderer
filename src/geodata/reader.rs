use errors::*;

use capnp::Word;
use capnp::serialize::SliceSegments;
use capnp::serialize;
use capnp::message::{Reader, ReaderOptions};
use geodata_capnp::geodata;
use memmap::{Mmap, Protection};
use owning_ref::{OwningHandle, OwningRef};

type GeodataHandle<'a> = OwningHandle<
    OwningRef<Box<Mmap>, [Word]>,
    OwningHandle<
        Box<Reader<SliceSegments<'a>>>,
        Box<geodata::Reader<'a>>
    >
>;

pub struct GeodataReader<'a> {
    handle: GeodataHandle<'a>,
}

impl<'a> GeodataReader<'a> {
    pub fn new(file_name: &str) -> Result<GeodataReader> {
        let input_file = Mmap::open_path(file_name, Protection::Read)
            .chain_err(|| format!("Failed to map {} to memory", file_name))?;

        let backing_storage = OwningRef::new(
            Box::new(input_file))
                .map(|x| Word::bytes_to_words(unsafe {x.as_slice()})
        );

        let handle = GeodataHandle::try_new(
            backing_storage,
            |x| {
                let message = serialize::read_message_from_words(
                    unsafe{&*x}.as_ref(),
                    ReaderOptions {
                        traversal_limit_in_words: u64::max_value(),
                        nesting_limit: i32::max_value(),
                    }
                )?;
                OwningHandle::try_new(
                    Box::new(message),
                    |y| unsafe{&*y}.get_root::<geodata::Reader>().map(Box::new)
                )
            }
        )
            .chain_err(|| format!("Failed to decode geodata from {}", file_name))?;

        Ok(GeodataReader {
            handle: handle,
        })
    }

    pub fn get_node_count(&self) -> u32 {
        self.get_reader().get_nodes().unwrap().len()
    }

    fn get_reader(&self) -> &geodata::Reader {
        &self.handle
    }
}
