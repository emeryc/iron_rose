#![no_main]
use iron_rose::IBF;
use libfuzzer_sys::fuzz_target;
use std::collections::HashSet;
use std::convert::TryInto;

fuzz_target!(|data: &[u8]| {
    let padding = [0; 16];
    let mut ibf = IBF::new(997);
    let set = data
        .chunks(16)
        .map(|chunk| {
            let padded = [&padding[..(16 - chunk.len())], chunk].concat();
            let chunk = padded.as_slice();
            u128::from_be_bytes(chunk.try_into().unwrap())
        })
        .collect::<HashSet<_>>();
    for d in set.iter() {
        let _ = ibf.encode(*d);
    }
    ibf.decode()
        .expect(format!("Should decode? {:#?}", set).as_str());
});
